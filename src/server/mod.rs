extern crate test;

mod http;

use std::cmp;
use std::io;
use std::io::Read;
use std::io::Result as IoResult;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter, BufRead};
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
    println!("\n{}\n", request);

    let response = http::Response::build(StatusCode::Ok,
            String::from("hello world"));
    println!("First Response:\n  {:?}", response);
    let response = format!("{}", response);
    writer.write(response.as_bytes())?;

    Ok(())
}

pub fn my_parse(request: &'static str) -> http::RequestHeader {
    let request = StringReader::new(request);
    http::RequestHeader::parse(&mut request.lines()).expect("couldn't parse")
}

mod tests {
    use super::*;
    use self::test::Bencher;

    #[bench]
    fn bench_my_parse(b: &mut Bencher) {
        let request =
            "GET / HTTP/1.1\r\n\
            Host: www.foo.bar\r\n\
            User-Agent: Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:60.0) Gecko/20100101 Firefox/60.0\r\n\
            Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n\
            Accept-Language: en-GB,en;q=0.5\r\n\
            Accept-Encoding: gzip, deflate\r\n\
            Connection: keep-alive\r\n\
            Upgrade-Insecure-Requests: 1\r\n\
            Cache-Control: max-age=0\r\n\
            \r\n";
        b.iter(|| my_parse(request));
    }
}

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
