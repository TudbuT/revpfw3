use std::env;

use revpfw3::{client, server, ClientParams};

fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    if (6..=10).contains(&args.len()) && args[0] == "client" {
        client(ClientParams {
            server_ip: &args[1],
            server_port: args[2].parse().unwrap(),
            dest_ip: &args[3],
            dest_port: args[4].parse().unwrap(),
            key: &args[5],
            sleep_delay_ms: if args.len() == 7 {
                args[6].parse().unwrap()
            } else {
                1
            },
            modem_port: args.get(7).map(|x| x.as_str()),
            modem_baud: args.get(8).map(|x| x.parse().unwrap()),
            modem_init: args.get(9).map(|x| x.as_str()),
        });
    }
    if (3..=4).contains(&args.len()) && args[0] == "server" {
        server(
            args[1].parse().unwrap(),
            &args[2],
            if args.len() == 4 {
                args[3].parse().unwrap()
            } else {
                1
            },
        );
    }
    eprintln!("Usage: \n\
               \x20 revpfw3 server <port> <key> [<poll delay>]\n\
               \x20 revpfw3 client <server ip> <server port> <destination ip> <destination port> <key> [<poll delay> [<modem port> <modem baud> <modem init>]]");
}
