//! Small, single-connection-at-a-time HTTP server prototype.
//! The request handler is kept separate from the accept loop so a bad client
//! can receive an error response without bringing down the listener.
mod http;

use self::http::{Body, Field, MessageBuilder, Method, Request, StatusCode};
use std::io::{self, BufReader, BufWriter, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

// The echo route holds the complete body in memory. This temporary ceiling
// keeps one client from asking us to allocate an arbitrarily large buffer;
// Git LFS object uploads will need to stream to storage instead.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Listen on loopback and keep accepting clients after a connection fails.
pub fn accept_connections(port: u16) {
    let listen_addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(listen_addr)
        .unwrap_or_else(|error| panic!("Failed to bind to {}: {}", listen_addr, error));

    println!("Listening on {}", listen_addr);
    // For now, each connection is handled before the next one is accepted.
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let addr = match stream.peer_addr() {
                    Ok(addr) => addr,
                    Err(error) => {
                        eprintln!("Could not identify client: {}", error);
                        continue;
                    }
                };
                if let Err(error) = handle_connection(addr, stream) {
                    eprintln!("Client {}: {}", addr, error);
                }
            }
            Err(error) => eprintln!("Accept failed: {}", error),
        }
    }
}

fn handle_connection(addr: SocketAddr, stream: TcpStream) -> io::Result<()> {
    println!("New client: {}", addr);
    // A client that stops sending mid-request must not block this sequential
    // listener forever. Persistent connections are not supported yet.
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);

    // Header parsing errors are client errors. Socket write failures still
    // return through io::Result for the accept loop to log and move past.
    let request = match Request::parse(&mut reader) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("Client {}: {}", addr, error);
            return write_response(
                &mut writer,
                StatusCode::BadRequest,
                b"Bad Request",
                "text/plain",
            );
        }
    };

    let body = match read_body(&request, &mut reader) {
        Ok(body) => body,
        Err((status, message)) => {
            return write_response(&mut writer, status, message.as_bytes(), "text/plain");
        }
    };

    let (status, response_body, content_type) = route(&request, body);
    write_response(&mut writer, status, &response_body, content_type)
}

fn read_body(
    request: &Request,
    reader: &mut BufReader<&TcpStream>,
) -> Result<Body, (StatusCode, &'static str)> {
    // The parser only understands a fixed byte count. Reject alternate
    // framing and 100-continue expectations before waiting for body bytes.
    if !request.header_values("Transfer-Encoding").is_empty() {
        return Err((
            StatusCode::NotImplemented,
            "Transfer-Encoding is not supported",
        ));
    }
    if !request.header_values("Expect").is_empty() {
        return Err((StatusCode::ExpectationFailed, "Expect is not supported"));
    }

    // Even equal duplicate lengths are rejected so there is only one clear
    // boundary between this request and any bytes that follow it.
    let lengths = request.header_values("Content-Length");
    if lengths.len() > 1 {
        return Err((StatusCode::BadRequest, "Multiple Content-Length headers"));
    }
    let length = match lengths.first() {
        Some(value) => value
            .parse::<usize>()
            .map_err(|_| (StatusCode::BadRequest, "Invalid Content-Length"))?,
        // POST /echo cannot know where its body ends without a length.
        None if request.method() == &Method::POST => {
            return Err((StatusCode::LengthRequired, "Content-Length is required"));
        }
        None => 0,
    };
    if length > MAX_BODY_BYTES {
        return Err((StatusCode::PayloadTooLarge, "Request body is too large"));
    }

    // read_exact consumes precisely this request's body, including non-UTF-8
    // bytes. A short body is reported as a malformed request.
    Body::parse(reader, length).map_err(|_| (StatusCode::BadRequest, "Incomplete request body"))
}

// These prototype routes exercise framing and binary responses. Git LFS
// batch and object routes will be added in later milestones.
fn route(request: &Request, body: Body) -> (StatusCode, Vec<u8>, &'static str) {
    match (request.method(), request.target()) {
        (&Method::GET, "/") => (StatusCode::Ok, b"hello world".to_vec(), "text/plain"),
        (&Method::POST, "/echo") => (
            StatusCode::Ok,
            body.into_bytes(),
            "application/octet-stream",
        ),
        (_, "/" | "/echo") => (
            StatusCode::MethodNotAllowed,
            b"Method Not Allowed".to_vec(),
            "text/plain",
        ),
        _ => (StatusCode::NotFound, b"Not Found".to_vec(), "text/plain"),
    }
}

fn write_response(
    writer: &mut BufWriter<&TcpStream>,
    status: StatusCode,
    body: &[u8],
    content_type: &str,
) -> io::Result<()> {
    // One response is written per connection. Its byte length makes the body
    // self-delimiting, and Connection: close tells HTTP/1.1 clients not to wait
    // for another response on this socket.
    let mut response = MessageBuilder::response(status);
    response
        .add_field(Field::new_contentlength(body.len()))
        .add_field2("Content-Type", content_type)
        .add_field2("Connection", "close")
        .add_body_bytes(body.to_vec());
    writer.write_all(&response.into_bytes())?;
    // Report buffered write errors here, while the accept loop can log them.
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::Shutdown;
    use std::thread;

    fn exchange(request: &[u8]) -> Vec<u8> {
        // Exercise the real TCP handler without starting the endless listener.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (stream, peer) = listener.accept().unwrap();
            handle_connection(peer, stream).unwrap();
        });
        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(request).unwrap();
        // Half-close the request side so a deliberately short body produces
        // EOF, while leaving the response side open for the server's reply.
        client.shutdown(Shutdown::Write).unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).unwrap();
        server.join().unwrap();
        response
    }

    fn response_parts(response: &[u8]) -> (&[u8], &[u8]) {
        let boundary = response
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .unwrap();
        (&response[..boundary], &response[boundary + 4..])
    }

    #[test]
    fn routes_requests_and_frames_responses() {
        let response = exchange(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        let (head, body) = response_parts(&response);
        assert!(head.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(head
            .windows(b"Content-Length: 11".len())
            .any(|part| part == b"Content-Length: 11"));
        assert!(head
            .windows(b"Connection: close".len())
            .any(|part| part == b"Connection: close"));
        assert_eq!(body, b"hello world");

        let response = exchange(b"GET /missing HTTP/1.1\r\n\r\n");
        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

        let response = exchange(b"POST / HTTP/1.1\r\nContent-Length: 0\r\n\r\n");
        assert!(response.starts_with(b"HTTP/1.1 405 Method Not Allowed\r\n"));
    }

    #[test]
    fn reads_and_echoes_exact_binary_body() {
        let response = exchange(b"POST /echo HTTP/1.1\r\ncontent-length: 4\r\n\r\n\x00\xffhi");
        let (head, body) = response_parts(&response);
        assert!(head.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(head
            .windows(b"Content-Length: 4".len())
            .any(|part| part == b"Content-Length: 4"));
        assert_eq!(body, b"\x00\xffhi");
    }

    #[test]
    fn rejects_bad_framing_without_panicking() {
        let cases: &[(&[u8], &[u8])] = &[
            (b"not an HTTP request\r\n\r\n", b"HTTP/1.1 400 Bad Request"),
            (
                b"POST /echo HTTP/1.1\r\n\r\n",
                b"HTTP/1.1 411 Length Required",
            ),
            (
                b"POST /echo HTTP/1.1\r\nContent-Length: nope\r\n\r\n",
                b"HTTP/1.1 400 Bad Request",
            ),
            (
                b"POST /echo HTTP/1.1\r\nContent-Length: 4\r\n\r\nhi",
                b"HTTP/1.1 400 Bad Request",
            ),
            (
                b"POST /echo HTTP/1.1\r\nContent-Length: 1\r\ncontent-length: 1\r\n\r\nx",
                b"HTTP/1.1 400 Bad Request",
            ),
            (
                b"POST /echo HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n",
                b"HTTP/1.1 501 Not Implemented",
            ),
        ];
        for &(request, status) in cases {
            let response = exchange(request);
            assert!(
                response.starts_with(status),
                "response: {:?}",
                String::from_utf8_lossy(&response)
            );
        }
    }
}
