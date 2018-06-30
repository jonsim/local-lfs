mod http;

use std::cmp;
use std::io;
use std::io::Read;
use std::io::Result as IoResult;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter, BufRead};
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


#[cfg(test)]
mod tests {
    // TODO: These tests should be in http/mod.rs
    use super::*;

    struct StringReader {
        content: String,
        pos: usize,
    }
    impl StringReader {
        pub fn new(content: &'static str) -> StringReader {
            StringReader{ content: String::from(content), pos: 0 }
        }
    }
    impl Read for StringReader {
        fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
            let len = cmp::min(buf.len(), self.content.len() - self.pos);
            let end = self.pos + len;
            buf.clone_from_slice(&self.content.as_bytes()[self.pos..end]);
            self.pos += len;
            Ok(len)
        }
    }
    impl BufRead for StringReader {
        fn fill_buf(&mut self) -> IoResult<&[u8]> {
            Ok(&self.content.as_bytes()[self.pos..])
        }

        fn consume(&mut self, amt: usize) {
            self.pos += amt;
        }
    }


    fn parse(request: &'static str) -> http::Request {
        let mut request = StringReader::new(request);
        http::Request::parse(&mut request).unwrap()
    }

    #[test]
    fn full_parse() {
        let request = parse(
            "GET / HTTP/1.1\r\n\
            Host: www.foo.bar\r\n\
            User-Agent: Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:60.0) Gecko/20100101 Firefox/60.0\r\n\
            Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n\
            Accept-Language: en-GB,en;q=0.5\r\n\
            Accept-Encoding: gzip, deflate\r\n\
            Connection: keep-alive\r\n\
            Upgrade-Insecure-Requests: 1\r\n\
            Cache-Control: max-age=0\r\n\
            \r\n"
        );
        assert_eq!(http::Method::GET, *request.method());
        assert_eq!("/",   request.target());
        assert_eq!(1, request.version().major());
        assert_eq!(1, request.version().minor());
    }
}
