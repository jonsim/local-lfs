mod http;

use std::io;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream, SocketAddr};
use self::http::status_code::StatusCode;

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

    let reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);

    // println!("Data:");
    // for line in reader.lines() {
    //     println!("  {}", line?);
    // }
    let line_iter = &mut reader.lines();
    let request = http::RequestHeader::parse(line_iter).expect("couldn't parse");
    println!("First Request:\n  {:?}", request);
    let response = http::Response::build(StatusCode::Ok,
            String::from("hello world"));
    println!("First Response:\n  {:?}", response);
    let response = format!("{}", response);
    writer.write(response.as_bytes())?;

    Ok(())
}
