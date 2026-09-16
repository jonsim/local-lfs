//! Small, single-connection-at-a-time HTTP server prototype.
//! The request handler is kept separate from the accept loop so a bad client
//! can receive an error response without bringing down the listener.
mod http;
mod store;

use self::http::{Body, Field, MessageBuilder, Method, Request, StatusCode};
use self::store::{ObjectStore, StoreError};
use std::io::{self, BufReader, BufWriter, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

// Ordinary prototype routes hold the body in memory, so they need a ceiling.
// PUT /objects streams directly to a temporary file and does not use this cap.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Listen on loopback and keep accepting clients after a connection fails.
pub fn accept_connections(port: u16, store_path: &str) {
    let store = ObjectStore::new(store_path)
        .unwrap_or_else(|error| panic!("Could not open store {}: {}", store_path, error));
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
                if let Err(error) = handle_connection(addr, stream, &store) {
                    eprintln!("Client {}: {}", addr, error);
                }
            }
            Err(error) => eprintln!("Accept failed: {}", error),
        }
    }
}

fn handle_connection(addr: SocketAddr, stream: TcpStream, store: &ObjectStore) -> io::Result<()> {
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

    let length = match request_body_length(&request) {
        Ok(length) => length,
        Err((status, message)) => {
            return write_response(&mut writer, status, message.as_bytes(), "text/plain");
        }
    };

    // Basic object transfer routes are already useful without the batch API.
    // PUT must bypass the in-memory body parser so large files can be stored.
    if let Some(oid) = request.target().strip_prefix("/objects/") {
        return handle_object_request(&request, oid, length, &mut reader, &mut writer, store);
    }

    let body = match read_body(length, &mut reader) {
        Ok(body) => body,
        Err((status, message)) => {
            return write_response(&mut writer, status, message.as_bytes(), "text/plain");
        }
    };

    let (status, response_body, content_type) = route(&request, body);
    write_response(&mut writer, status, &response_body, content_type)
}

fn request_body_length(request: &Request) -> Result<usize, (StatusCode, &'static str)> {
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
        // Uploads and POST /echo cannot know where the body ends without it.
        None if request.method() == &Method::POST || request.method() == &Method::PUT => {
            return Err((StatusCode::LengthRequired, "Content-Length is required"));
        }
        None => 0,
    };
    Ok(length)
}

fn read_body(
    length: usize,
    reader: &mut BufReader<&TcpStream>,
) -> Result<Body, (StatusCode, &'static str)> {
    if length > MAX_BODY_BYTES {
        return Err((StatusCode::PayloadTooLarge, "Request body is too large"));
    }
    // read_exact consumes precisely this request's body, including non-UTF-8
    // bytes. A short body is reported as a malformed request.
    Body::parse(reader, length).map_err(|_| (StatusCode::BadRequest, "Incomplete request body"))
}

fn handle_object_request(
    request: &Request,
    oid: &str,
    length: usize,
    reader: &mut BufReader<&TcpStream>,
    writer: &mut BufWriter<&TcpStream>,
    store: &ObjectStore,
) -> io::Result<()> {
    match request.method() {
        Method::PUT => {
            // Content-Length is the upload's claimed size. The store counts
            // bytes as they arrive and checks SHA-256 before publishing them.
            match store.put(oid, length as u64, reader) {
                Ok(()) => write_response(writer, StatusCode::Ok, b"", "text/plain"),
                Err(error) => write_store_error(writer, error),
            }
        }
        Method::GET => {
            // GET uses the file handle directly rather than buffering the
            // whole object. The metadata length frames the streamed response.
            match store.open(oid) {
                Ok((mut file, size)) => {
                    let mut response = MessageBuilder::response(StatusCode::Ok);
                    response
                        .add_field2("Content-Length", &size.to_string())
                        .add_field2("Content-Type", "application/octet-stream")
                        .add_field2("Connection", "close");
                    writer.write_all(&response.into_bytes())?;
                    io::copy(&mut file, writer)?;
                    writer.flush()
                }
                Err(error) => write_store_error(writer, error),
            }
        }
        _ => write_response(
            writer,
            StatusCode::MethodNotAllowed,
            b"Method Not Allowed",
            "text/plain",
        ),
    }
}

fn write_store_error(writer: &mut BufWriter<&TcpStream>, error: StoreError) -> io::Result<()> {
    eprintln!("Object request failed: {}", error);
    // Client errors can explain the rejected upload. Internal filesystem
    // paths stay in the server log rather than the HTTP response body.
    let (status, message) = match &error {
        StoreError::InvalidOid => (StatusCode::BadRequest, error.to_string()),
        StoreError::NotFound => (StatusCode::NotFound, error.to_string()),
        StoreError::SizeMismatch { .. } | StoreError::HashMismatch => {
            (StatusCode::UnprocessableEntity, error.to_string())
        }
        StoreError::Read(read_error)
            if matches!(
                read_error.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) =>
        {
            (StatusCode::RequestTimeout, "Request Timeout".to_string())
        }
        StoreError::Read(_) => (StatusCode::BadRequest, "Incomplete upload".to_string()),
        StoreError::Io(_) => (
            StatusCode::InternalServerError,
            "Internal Server Error".to_string(),
        ),
    };
    write_response(writer, status, message.as_bytes(), "text/plain")
}

// These prototype routes exercise framing and binary responses. The object
// transfer routes are handled above; batch negotiation comes later.
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
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    use std::net::Shutdown;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "local-lfs-http-test-{}-{}",
                std::process::id(),
                id
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn exchange_in_store(request: &[u8], store_path: &Path) -> Vec<u8> {
        // Exercise the real TCP handler without starting the endless listener.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let store = ObjectStore::new(store_path).unwrap();
        let server = thread::spawn(move || {
            let (stream, peer) = listener.accept().unwrap();
            handle_connection(peer, stream, &store).unwrap();
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

    fn exchange(request: &[u8]) -> Vec<u8> {
        let directory = TestDirectory::new();
        exchange_in_store(request, &directory.0)
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

    #[test]
    fn uploads_and_downloads_persistent_raw_object() {
        let directory = TestDirectory::new();
        // This exceeds the echo route's memory cap, proving the object path
        // streams the upload instead of trying to buffer it as a Body.
        let bytes = vec![0xa5; MAX_BODY_BYTES + 1];
        let oid = hex::encode(Sha256::digest(&bytes));
        let mut upload = format!(
            "PUT /objects/{} HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            oid,
            bytes.len()
        )
        .into_bytes();
        upload.extend_from_slice(&bytes);
        let response = exchange_in_store(&upload, &directory.0);
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));

        let download = format!("GET /objects/{} HTTP/1.1\r\n\r\n", oid);
        let response = exchange_in_store(download.as_bytes(), &directory.0);
        let (head, body) = response_parts(&response);
        assert!(head.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(head
            .windows(format!("Content-Length: {}", bytes.len()).len())
            .any(|part| part == format!("Content-Length: {}", bytes.len()).as_bytes()));
        assert_eq!(body, bytes);
    }

    #[test]
    fn rejects_invalid_object_uploads() {
        let directory = TestDirectory::new();
        let oid = hex::encode(Sha256::digest(b"expected"));
        let request = format!(
            "PUT /objects/{} HTTP/1.1\r\nContent-Length: 5\r\n\r\nwrong",
            oid
        );
        let response = exchange_in_store(request.as_bytes(), &directory.0);
        assert!(response.starts_with(b"HTTP/1.1 422 Unprocessable Entity\r\n"));

        let missing = format!("GET /objects/{} HTTP/1.1\r\n\r\n", oid);
        let response = exchange_in_store(missing.as_bytes(), &directory.0);
        assert!(response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

        let bad_oid = exchange_in_store(
            b"PUT /objects/../escape HTTP/1.1\r\nContent-Length: 0\r\n\r\n",
            &directory.0,
        );
        assert!(bad_oid.starts_with(b"HTTP/1.1 400 Bad Request\r\n"));

        let short = format!(
            "PUT /objects/{} HTTP/1.1\r\nContent-Length: 10\r\n\r\nhi",
            oid
        );
        let response = exchange_in_store(short.as_bytes(), &directory.0);
        assert!(response.starts_with(b"HTTP/1.1 422 Unprocessable Entity\r\n"));
    }
}
