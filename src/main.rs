use std::env;

use revpfw3::{client, server};

fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    if (6..=7).contains(&args.len()) && args[0] == "client" {
        client(
            &args[1],
            args[2].parse().unwrap(),
            &args[3],
            args[4].parse().unwrap(),
            &args[5],
            if args.len() == 7 { args[6].parse().unwrap() } else { 1 }
        );
    }
    if (3..=4).contains(&args.len()) && args[0] == "server" {
        server(args[1].parse().unwrap(), &args[2], if args.len() == 4 { args[3].parse().unwrap() } else { 1 });
    }
    eprintln!("Usage: \n\
               \x20 revpfw3 server <port> <key> [<poll delay>]\n\
               \x20 revpfw3 client <server ip> <server port> <destination ip> <destination port> <key> [<poll delay>]");
}
