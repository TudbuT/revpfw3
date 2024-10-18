use std::{
    collections::HashMap,
    io::Read,
    io::Write,
    net::{Shutdown, TcpListener},
    thread,
    time::{Duration, SystemTime},
    vec,
};

use crate::{Connection, PacketType, SocketAdapter};

fn resync(tcp: &mut SocketAdapter) {
    println!();
    eprintln!("Client version mismatch or broken connection. Re-syncing in case of the latter...");
    tcp.internal.set_print(false);
    tcp.write_now().unwrap();
    tcp.write(&[PacketType::Resync.ordinal() as u8]).unwrap();
    tcp.write_now().unwrap();
    eprintln!(
        "Sent resync packet. Client should now wait 8 seconds and then send a resync packet back, initiating a normal re-sync."
    );
    let mut buf = [0; 4096];
    // read all packets that are still pending.
    while let Some(Some(_x @ 1..)) = tcp.poll(&mut buf).ok() {}
    // wait 5 seconds
    thread::sleep(Duration::from_secs(5));
    // read all packets that are still pending.
    while let Some(Some(_x @ 1..)) = tcp.poll(&mut buf).ok() {}
    // client should now have stopped sending packets.
}

pub fn server(port: u16, key: &str, sleep_delay_ms: u64) {
    let mut buf1 = [0u8; 1];
    let mut buf4 = [0u8; 4];
    let mut buf8 = [0u8; 8];
    let mut buf16 = [0u8; 16];
    let mut buf = [0; 1024];
    let tcpl = TcpListener::bind(("::1", port)).unwrap();
    let mut tcp = loop {
        let Ok(mut tcp) = tcpl.accept() else { continue };
        let Ok(()) = tcp.0.read_exact(&mut buf4) else {
            tcp.0.shutdown(Shutdown::Both).unwrap();
            continue;
        };
        if buf4 == [b'R', b'P', b'F', 30] {
            println!("Compatible client connected.");
            if tcp.0.read_exact(&mut buf4).is_ok() && u32::from_be_bytes(buf4) == key.len() as u32 {
                println!("Key length matches.");
                let mut keybuf = vec![0u8; key.len()];
                if tcp.0.read_exact(&mut keybuf).is_ok() && keybuf == key.as_bytes() {
                    println!("Accepted.");
                    break tcp.0;
                }
                println!("Key content does not match.");
            }
            println!("Key mismatch - forgetting client.");
        }
    };

    tcp.write_all(&[b'R', b'P', b'F', 30]).unwrap();

    tcpl.set_nonblocking(true).unwrap();

    let mut tcp = SocketAdapter::new(Connection::new_tcp(tcp, true));
    let mut sockets: HashMap<u64, SocketAdapter> = HashMap::new();
    let mut id = 0;
    let mut last_keep_alive_sent = SystemTime::now();
    let mut last_keep_alive = SystemTime::now();
    loop {
        let mut did_anything = false;

        if last_keep_alive_sent.elapsed().unwrap().as_secs() >= 10 {
            last_keep_alive_sent = SystemTime::now();
            tcp.write(&[PacketType::KeepAlive.ordinal() as u8]).unwrap();
        }
        if last_keep_alive.elapsed().unwrap().as_secs() >= 60 {
            panic!("connection dropped. exiting.");
        }

        if let Ok(new) = tcpl.accept() {
            let new = SocketAdapter::new(Connection::new_tcp(new.0, false));
            sockets.insert((id, id += 1).0, new);
            tcp.write(&[PacketType::NewClient.ordinal() as u8]).unwrap();
            did_anything = true;
        }

        let mut to_remove = vec![];
        for (&i, socket) in sockets.iter_mut() {
            if let Ok(x) = socket.poll(&mut buf) {
                if let Some(len) = x {
                    if len == 0 {
                        to_remove.push(i);
                    } else {
                        tcp.write(&[PacketType::ClientData.ordinal() as u8])
                            .unwrap();
                        tcp.write(&i.to_be_bytes()).unwrap();
                        tcp.write(&(len as u32).to_be_bytes()).unwrap();
                        tcp.write(&buf[..len]).unwrap();
                    }
                    did_anything = true;
                }
            } else {
                to_remove.push(i);
                did_anything = true;
            }
            if let x @ 1.. = socket.clear_delay() {
                tcp.write(&[PacketType::ClientExceededBuffer.ordinal() as u8])
                    .unwrap();
                tcp.write(&i.to_be_bytes()).unwrap();
                tcp.write(&x.to_be_bytes()).unwrap();
                socket.punish(x);
            }
        }
        for i in to_remove.into_iter().rev() {
            tcp.write(&[PacketType::CloseClient.ordinal() as u8])
                .unwrap();
            tcp.write(&i.to_be_bytes()).unwrap();
            if let Some(x) = sockets.remove(&i) {
                let _ = x.internal.close();
            }
        }

        tcp.update().unwrap();
        if tcp.poll_exact(&mut buf1).unwrap().is_none() {
            if !did_anything {
                thread::sleep(Duration::from_millis(sleep_delay_ms));
            }
            continue;
        }

        let Some(pt) = PacketType::from_ordinal(buf1[0] as i8) else {
            resync(&mut tcp);
            continue;
        };
        match pt {
            PacketType::NewClient => resync(&mut tcp),

            PacketType::CloseClient => {
                tcp.read_now(&mut buf8).unwrap();
                if let Some(x) = sockets.remove(&u64::from_be_bytes(buf8)) {
                    let _ = x.internal.close();
                }
            }

            PacketType::KeepAlive => {
                last_keep_alive = SystemTime::now();
            }

            PacketType::ClientData => resync(&mut tcp),

            PacketType::ServerData => {
                tcp.read_now(&mut buf8).unwrap();
                let idx = u64::from_be_bytes(buf8);
                tcp.read_now(&mut buf4).unwrap();
                let len = u32::from_be_bytes(buf4) as usize;
                tcp.read_now(&mut buf[..len]).unwrap();

                if let Some(socket) = sockets.get_mut(&idx) {
                    let _ = socket.write_later(&buf[..len]);
                }
            }

            PacketType::ClientExceededBuffer => {
                tcp.read_now(&mut buf8).unwrap();
                let idx = u64::from_be_bytes(buf8);
                tcp.read_now(&mut buf16).unwrap();
                let amount = u128::from_be_bytes(buf16);

                // a single connection doesn't need overuse-penalties
                if let (true, Some(socket)) = (sockets.len() > 1, sockets.get_mut(&idx)) {
                    socket.punish(amount);
                }
            }

            PacketType::Resync => {
                println!();
                tcp.internal.set_print(false);
                eprintln!(
                    "Client asked for a re-sync. Waiting 8 seconds, then sending resync-echo."
                );
                tcp.read_now(&mut buf8).unwrap();
                id = u64::from_be_bytes(buf8).max(id);
                tcp.write_now().unwrap();
                thread::sleep(Duration::from_secs(8));
                tcp.write(&[PacketType::ResyncEcho.ordinal() as u8])
                    .unwrap();
                tcp.write(&id.to_be_bytes()).unwrap();
                tcp.write_now().unwrap();
                eprintln!("Resync-Echo sent. Going back to normal operation.");
                tcp.internal.set_print(true);
            }

            // this one can't happen, it should only come from the server
            PacketType::ResyncEcho => resync(&mut tcp),
        }
    }
}
