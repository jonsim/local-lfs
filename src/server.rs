use std::fmt;
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::net::{TcpListener, TcpStream, SocketAddr};

pub fn accept_connections(port: i32) {
    let listen_addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(listen_addr.clone()).expect(&format!(
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

struct HttpRequestLine {
    version_major: u8,
    version_minor: u8,
    method: String,
    target: String,
}

struct HttpResponseLine {
    version_major: u8,
    version_minor: u8,
    status: u16,
    reason: String,
}

impl HttpRequestLine {
    fn read(start_line: String) -> Result<HttpRequestLine, &'static str> {
        let mut parts = start_line.split(' ');
        let method = String::from(parts.next().ok_or("Bad HTTP request")?);
        let target = String::from(parts.next().ok_or("Bad HTTP request")?);
        let vsn: Vec<u8> = parts.next().ok_or("Bad HTTP request")?.bytes().collect();

        if vsn.len() != 8 {
            return Err("Bad HTTP version");
        }
        let version_major: u8 = vsn[5] - b'0';
        let version_minor: u8 = vsn[7] - b'0';
        if version_major > 9 || version_minor > 9 {
            return Err("Bad HTTP version");
        }

        Ok(HttpRequestLine{ version_major, version_minor, method, target })
    }

    fn to_string(&self) -> String {
        format!("{} {} HTTP-{}.{}", self.method, self.target, self.version_major,
                self.version_minor)
    }
}

impl fmt::Debug for HttpRequestLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "HTTP-Request {{ {:?}, {:?}, HTTP-{:?}.{:?} }}",
                self.method, self.target, self.version_major, self.version_minor)
    }
}
