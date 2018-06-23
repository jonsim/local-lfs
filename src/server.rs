use std::net::TcpListener;

pub fn start(port: i32) {
    let listen_addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(listen_addr.clone()).expect(&format!(
        "Failed to bind to {}", listen_addr));

    println!("Listening on {}", listen_addr);
    loop {
        match listener.accept() {
            Ok((stream, addr)) => {
                println!("New client: {}", addr);
            }
            Err(error) => {
                println!("Accept failed {}", error); // TODO: Log
                continue;
            }
        }
    }
}
