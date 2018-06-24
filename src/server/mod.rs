mod http;

use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::{TcpListener, TcpStream, SocketAddr};

pub fn accept_connections(port: u16) {
    let listen_addr = SocketAddr::from(([127,0,0,1], port));
    let listener = TcpListener::bind(listen_addr).expect(&format!(
        "Failed to bind to {}", listen_addr));

    println!("Listening on {}", listen_addr);
    loop {
        match listener.accept() {
            Ok((stream, addr)) => {
                // TODO: Threadify.
                handle_connection(addr, stream).expect("failed to handle");
            }
            Err(error) => {
                // TODO: Log.
                println!("Accept failed {}", error);
                continue;
            }
        }
    }
}

fn handle_connection(addr: SocketAddr, stream: TcpStream) -> io::Result<()> {
    println!("New client: {}", addr);

    let reader = BufReader::new(stream);

    println!("Data:");
    for line in reader.lines() {
        println!("  {}", line?);
    }

    Ok(())
}
