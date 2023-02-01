use std::{
    io::Read,
    io::Write,
    net::{Shutdown, TcpStream},
    thread,
    time::{Duration, SystemTime},
    vec, collections::HashMap,
};

use crate::{io_sync, PacketType, SocketAdapter};

pub fn client(ip: &str, port: u16, dest_ip: &str, dest_port: u16, key: &str, sleep_delay_ms: u64) {
    let mut buf1 = [0u8; 1];
    let mut buf4 = [0u8; 4];
    let mut buf8 = [0u8; 8];
    let mut buf16 = [0u8; 16];
    let mut buf = [0; 1024];
    let mut tcp = TcpStream::connect((ip, port)).unwrap();
    println!("Syncing...");
    tcp.write_all(&['R' as u8, 'P' as u8, 'F' as u8, 30])
        .unwrap();
    println!("Authenticating...");
    tcp.write_all(&(key.len() as u32).to_be_bytes()).unwrap();
    tcp.write_all(key.as_bytes()).unwrap();

    println!("Syncing...");
    tcp.read_exact(&mut buf4).unwrap();
    if buf4 != ['R' as u8, 'P' as u8, 'F' as u8, 30] {
        panic!("RPF30 header expected, but not found. Make sure the server is actually running revpfw3!");
    }
    tcp.write_all(&[PacketType::KeepAlive.ordinal() as u8])
        .unwrap();

    println!("READY!");

    let mut tcp = SocketAdapter::new(tcp);
    tcp.set_nonblocking(true);
    let mut sockets: HashMap<u64, SocketAdapter> = HashMap::new();
    let mut id = 0;
    let mut last_keep_alive = SystemTime::now();
    loop {
        let mut did_anything = false;

        if last_keep_alive.elapsed().unwrap().as_secs() >= 60 {
            panic!("connection dropped. exiting.");
        }

        let mut to_remove = vec![];
        for (&i, socket) in sockets.iter_mut() {
            if let Ok(x) = socket.poll(&mut buf) {
                if let Some(len) = x {
                    if len == 0 {
                        to_remove.push(i);
                    } else {
                        tcp.write(&[PacketType::ServerData.ordinal() as u8])
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
            }
        }
        for i in to_remove.into_iter().rev() {
            tcp.write(&[PacketType::CloseClient.ordinal() as u8])
                .unwrap();
            tcp.write(&i.to_be_bytes()).unwrap();
            if let Some(x) = sockets.remove(&i) {
                let _ = x.internal.shutdown(Shutdown::Both);
            }
        }

        tcp.update().unwrap();
        if io_sync(tcp.internal.read_exact(&mut buf1))
            .unwrap()
            .is_none()
        {
            if !did_anything {
                thread::sleep(Duration::from_millis(sleep_delay_ms));
            }
            continue;
        }

        let pt = PacketType::from_ordinal(buf1[0] as i8)
            .expect("server/client version mismatch or broken TCP");
        tcp.set_nonblocking(false);
        match pt {
            PacketType::NewClient => {
                let mut tcp = SocketAdapter::new(TcpStream::connect((dest_ip, dest_port)).unwrap());
                tcp.set_nonblocking(true);
                sockets.insert((id, id += 1).0, tcp);
            }

            PacketType::CloseClient => {
                tcp.internal.read_exact(&mut buf8).unwrap();
                if let Some(x) = sockets.remove(&u64::from_be_bytes(buf8)) {
                    let _ = x.internal.shutdown(Shutdown::Both);
                }
            }

            PacketType::KeepAlive => {
                last_keep_alive = SystemTime::now();
                tcp.write(&[PacketType::KeepAlive.ordinal() as u8]).unwrap();
            }

            PacketType::ClientData => {
                tcp.internal.read_exact(&mut buf8).unwrap();
                let idx = u64::from_be_bytes(buf8);
                tcp.internal.read_exact(&mut buf4).unwrap();
                let len = u32::from_be_bytes(buf4) as usize;
                tcp.internal.read_exact(&mut buf[..len]).unwrap();

                if let Some(socket) = sockets.get_mut(&idx) {
                    let _ = socket.write_later(&buf[..len]);
                }
            }

            PacketType::ServerData => unreachable!(),

            PacketType::ClientExceededBuffer => {
                tcp.internal.read_exact(&mut buf8).unwrap();
                let idx = u64::from_be_bytes(buf8);
                tcp.internal.read_exact(&mut buf16).unwrap();
                let amount = u128::from_be_bytes(buf16);

                // a single connection doesn't need overuse-penalties
                if let (true, Some(socket)) = (sockets.len() > 1, sockets.get_mut(&idx)) {
                    socket.punish(amount);
                }
            }
        }
        tcp.set_nonblocking(true);
    }
}
