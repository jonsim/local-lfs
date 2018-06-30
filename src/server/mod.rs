mod http;

use std::io;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream, SocketAddr};
use self::http::StatusCode;

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

    let mut reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);

    let request = http::Request::parse(&mut reader).expect("couldn't parse");
    println!("First Request:\n  {}", request);
    let body = http::Body::parse(&mut reader, 0).expect("couldn't parse");
    println!("First Body:\n  {}", body);

    let response_body = String::from("hello world");
    let mut response = http::MessageBuilder::response(StatusCode::Ok);
    response.add_field(http::Field::ContentLength(response_body.len()))
            .add_body(response_body);
    // println!("First Response:\n  {:?}", response);
    // let response = format!("{}\r\n{}", response_head, response_body);
    writer.write(&response.into_bytes())?;

    Ok(())
}
