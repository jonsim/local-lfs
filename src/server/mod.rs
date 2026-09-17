//! Small loopback HTTP server with a bounded number of active connections.
//! The request handler is kept separate from the accept loop so a bad client
//! can receive an error response without bringing down the listener.
mod batch;
mod http;
mod store;

use self::http::{Body, Field, MessageBuilder, Method, Request, StatusCode};
use self::store::{ObjectStore, StoreError};
use std::io::{self, BufReader, BufWriter, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

// Ordinary prototype routes hold the body in memory, so they need a ceiling.
// PUT /objects streams directly to a temporary file and does not use this cap.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const LFS_JSON: &str = "application/vnd.git-lfs+json";
const WORKERS: usize = 4;
const QUEUED_CONNECTIONS: usize = 16;

/// Listen on loopback and keep accepting clients after a connection fails.
pub fn accept_connections(port: u16, store_path: &str) {
    let store = ObjectStore::new(store_path)
        .unwrap_or_else(|error| panic!("Could not open store {}: {}", store_path, error));
    let listen_addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(listen_addr)
        .unwrap_or_else(|error| panic!("Failed to bind to {}: {}", listen_addr, error));

    println!("Listening on {}", listen_addr);
    // A bounded queue and fixed workers keep a stalled upload from delaying
    // every other client without creating a thread for every incoming socket.
    let (sender, receiver) = mpsc::sync_channel(QUEUED_CONNECTIONS);
    let receiver = Arc::new(Mutex::new(receiver));
    let store = Arc::new(store);
    for _ in 0..WORKERS {
        let receiver = Arc::clone(&receiver);
        let store = Arc::clone(&store);
        thread::spawn(move || loop {
            // Hold the receiver lock only while taking the next socket; each
            // worker then processes its request independently.
            let connection = receiver.lock().expect("connection queue poisoned").recv();
            match connection {
                Ok(stream) => serve_connection(stream, &store),
                Err(_) => break,
            }
        });
    }
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                if sender.send(stream).is_err() {
                    eprintln!("All connection workers have stopped");
                    break;
                }
            }
            Err(error) => eprintln!("Accept failed: {}", error),
        }
    }
}

fn serve_connection(stream: TcpStream, store: &ObjectStore) {
    let addr = match stream.peer_addr() {
        Ok(addr) => addr,
        Err(error) => {
            eprintln!("Could not identify client: {}", error);
            return;
        }
    };
    if let Err(error) = handle_connection(addr, stream, store) {
        eprintln!("Client {}: {}", addr, error);
    }
}

fn handle_connection(addr: SocketAddr, stream: TcpStream, store: &ObjectStore) -> io::Result<()> {
    println!("New client: {}", addr);
    // A client that stops sending mid-request must not occupy one worker
    // forever. Persistent connections are not supported yet.
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
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
            if request.target() == "/objects/batch" {
                return write_batch_error(&mut writer, status, message);
            }
            return write_response(&mut writer, status, message.as_bytes(), "text/plain");
        }
    };
    let expects_continue = match request_expect_continue(&request) {
        Ok(value) => value,
        Err((status, message)) => {
            if request.target() == "/objects/batch" {
                return write_batch_error(&mut writer, status, message);
            }
            return write_response(&mut writer, status, message.as_bytes(), "text/plain");
        }
    };
    let streaming_upload =
        request.method() == &Method::PUT && request.target().starts_with("/objects/");
    if length > MAX_BODY_BYTES && !streaming_upload {
        if request.target() == "/objects/batch" {
            return write_batch_error(
                &mut writer,
                StatusCode::PayloadTooLarge,
                "Request body is too large",
            );
        }
        return write_response(
            &mut writer,
            StatusCode::PayloadTooLarge,
            b"Request body is too large",
            "text/plain",
        );
    }
    if expects_continue {
        // Send the interim response before reading bytes from a client that
        // waits for permission to transmit its fixed-length request body.
        writer.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
        writer.flush()?;
    }

    // /objects/batch must be checked before the /objects/{oid} prefix route.
    if request.target() == "/objects/batch" {
        return handle_batch_request(
            &request,
            length,
            &mut reader,
            &mut writer,
            store,
            stream.local_addr()?,
        );
    }

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
    // framing before waiting for body bytes.
    if !request.header_values("Transfer-Encoding").is_empty() {
        return Err((
            StatusCode::NotImplemented,
            "Transfer-Encoding is not supported",
        ));
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

fn request_expect_continue(request: &Request) -> Result<bool, (StatusCode, &'static str)> {
    let expectations = request.header_values("Expect");
    match expectations.as_slice() {
        [] => Ok(false),
        [value]
            if (request.method() == &Method::POST || request.method() == &Method::PUT)
                && value.trim().eq_ignore_ascii_case("100-continue") =>
        {
            Ok(true)
        }
        _ => Err((StatusCode::ExpectationFailed, "Unsupported Expect header")),
    }
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

fn handle_batch_request(
    request: &Request,
    length: usize,
    reader: &mut BufReader<&TcpStream>,
    writer: &mut BufWriter<&TcpStream>,
    store: &ObjectStore,
    local_addr: SocketAddr,
) -> io::Result<()> {
    if request.method() != &Method::POST {
        return write_batch_error(
            writer,
            StatusCode::MethodNotAllowed,
            "Batch requests must use POST",
        );
    }

    // The Batch API uses a vendor JSON media type. A charset parameter on
    // Content-Type is allowed, and Accept can contain a list of media types.
    let content_types = request.header_values("Content-Type");
    if content_types.len() != 1 || !is_lfs_json(content_types[0]) {
        return write_batch_error(
            writer,
            StatusCode::UnsupportedMediaType,
            "Expected Git LFS JSON content type",
        );
    }
    let accepts = request.header_values("Accept");
    if !accepts
        .iter()
        .any(|value| value.split(',').any(is_lfs_json))
    {
        return write_batch_error(
            writer,
            StatusCode::NotAcceptable,
            "Git LFS JSON must be accepted",
        );
    }

    let body = match read_body(length, reader) {
        Ok(body) => body,
        Err((status, message)) => return write_batch_error(writer, status, message),
    };
    // This listener is loopback-only; action URLs use its actual local port
    // instead of trusting a client-supplied Host header.
    let base_url = format!("http://{}", local_addr);
    match batch::process(&body.into_bytes(), store, &base_url) {
        Ok(response) => write_response(writer, StatusCode::Ok, &response, LFS_JSON),
        Err(error) => write_batch_error(writer, error.status, error.message),
    }
}

fn is_lfs_json(value: &str) -> bool {
    value
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case(LFS_JSON)
}

fn write_batch_error(
    writer: &mut BufWriter<&TcpStream>,
    status: StatusCode,
    message: &str,
) -> io::Result<()> {
    let body = batch::error_body(message);
    write_response(writer, status, &body, LFS_JSON)
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

// These prototype routes exercise framing and binary responses. The Git LFS
// batch and object transfer routes are handled above.
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
        let response = exchange_at(addr, request);
        server.join().unwrap();
        response
    }

    fn exchange_at(addr: SocketAddr, request: &[u8]) -> Vec<u8> {
        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(request).unwrap();
        // Half-close the request side so a deliberately short body produces
        // EOF, while leaving the response side open for the server's reply.
        client.shutdown(Shutdown::Write).unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).unwrap();
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

    fn batch_request(json: &str) -> Vec<u8> {
        format!(
            "POST /objects/batch HTTP/1.1\r\nAccept: {}\r\nContent-Type: {}; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
            LFS_JSON,
            LFS_JSON,
            json.len(),
            json
        )
        .into_bytes()
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
    fn continues_a_waiting_object_upload() {
        let directory = TestDirectory::new();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let store = ObjectStore::new(&directory.0).unwrap();
        let server = thread::spawn(move || {
            let (stream, peer) = listener.accept().unwrap();
            handle_connection(peer, stream, &store).unwrap();
        });

        let bytes = b"wait for continue before uploading";
        let oid = hex::encode(Sha256::digest(bytes));
        let mut client = TcpStream::connect(addr).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client
            .write_all(
                format!(
                    "PUT /objects/{} HTTP/1.1\r\nExpect: 100-continue\r\nContent-Length: {}\r\n\r\n",
                    oid,
                    bytes.len()
                )
                .as_bytes(),
            )
            .unwrap();
        let mut interim = [0_u8; b"HTTP/1.1 100 Continue\r\n\r\n".len()];
        client.read_exact(&mut interim).unwrap();
        assert_eq!(&interim, b"HTTP/1.1 100 Continue\r\n\r\n");

        client.write_all(bytes).unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let mut final_response = Vec::new();
        client.read_to_end(&mut final_response).unwrap();
        assert!(final_response.starts_with(b"HTTP/1.1 200 OK\r\n"));
        server.join().unwrap();

        let store = ObjectStore::new(&directory.0).unwrap();
        let (mut file, _) = store.open(&oid).unwrap();
        let mut stored = Vec::new();
        file.read_to_end(&mut stored).unwrap();
        assert_eq!(stored, bytes);
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
            (
                b"POST /echo HTTP/1.1\r\nExpect: unsupported\r\nContent-Length: 0\r\n\r\n",
                b"HTTP/1.1 417 Expectation Failed",
            ),
            (
                b"POST /echo HTTP/1.1\r\nExpect: 100-continue\r\nContent-Length: 16777217\r\n\r\n",
                b"HTTP/1.1 413 Payload Too Large",
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

    #[test]
    fn batch_actions_complete_basic_upload_and_download() {
        let directory = TestDirectory::new();
        let store = ObjectStore::new(&directory.0).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            // Keep one test listener alive so every advertised action URL can
            // be used against the same server and object store.
            for _ in 0..5 {
                let (stream, peer) = listener.accept().unwrap();
                handle_connection(peer, stream, &store).unwrap();
            }
        });

        let bytes = b"a Git LFS object\x00\xff";
        let oid = hex::encode(Sha256::digest(bytes));
        let claim = format!(
            r#"{{"operation":"upload","objects":[{{"oid":"{}","size":{}}}]}}"#,
            oid,
            bytes.len()
        );
        let response = exchange_at(addr, &batch_request(&claim));
        let (head, body) = response_parts(&response);
        assert!(head.starts_with(b"HTTP/1.1 200 OK\r\n"));
        assert!(head
            .windows(LFS_JSON.len())
            .any(|part| part == LFS_JSON.as_bytes()));
        let json: serde_json::Value = serde_json::from_slice(body).unwrap();
        assert_eq!(json["transfer"], "basic");
        assert_eq!(json["hash_algo"], "sha256");
        assert_eq!(json["objects"][0]["oid"], oid);
        assert_eq!(json["objects"][0]["size"], bytes.len());
        let upload_href = json["objects"][0]["actions"]["upload"]["href"]
            .as_str()
            .unwrap();
        let base = format!("http://{}", addr);
        let upload_path = upload_href.strip_prefix(&base).unwrap();
        assert_eq!(upload_path, format!("/objects/{}", oid));

        let mut upload = format!(
            "PUT {} HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            upload_path,
            bytes.len()
        )
        .into_bytes();
        upload.extend_from_slice(bytes);
        let response = exchange_at(addr, &upload);
        assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));

        // A second upload batch omits actions because the object already exists.
        let response = exchange_at(addr, &batch_request(&claim));
        let (_, body) = response_parts(&response);
        let json: serde_json::Value = serde_json::from_slice(body).unwrap();
        assert!(json["objects"][0].get("actions").is_none());

        let claim = format!(
            r#"{{"operation":"download","transfers":["basic"],"objects":[{{"oid":"{}","size":{}}}]}}"#,
            oid,
            bytes.len()
        );
        let response = exchange_at(addr, &batch_request(&claim));
        let (_, body) = response_parts(&response);
        let json: serde_json::Value = serde_json::from_slice(body).unwrap();
        let download_href = json["objects"][0]["actions"]["download"]["href"]
            .as_str()
            .unwrap();
        let download_path = download_href.strip_prefix(&base).unwrap();
        assert_eq!(download_path, upload_path);

        let download = format!("GET {} HTTP/1.1\r\n\r\n", download_path);
        let response = exchange_at(addr, download.as_bytes());
        let (_, body) = response_parts(&response);
        assert_eq!(body, bytes);
        server.join().unwrap();
    }

    #[test]
    fn batch_reports_object_and_request_errors_as_json() {
        let directory = TestDirectory::new();
        let oid = hex::encode(Sha256::digest(b"missing"));
        let claim = format!(
            r#"{{"operation":"download","objects":[{{"oid":"{}","size":7}},{{"oid":"bad","size":1}}]}}"#,
            oid
        );
        let response = exchange_in_store(&batch_request(&claim), &directory.0);
        let (head, body) = response_parts(&response);
        assert!(head.starts_with(b"HTTP/1.1 200 OK\r\n"));
        let json: serde_json::Value = serde_json::from_slice(body).unwrap();
        assert_eq!(json["objects"][0]["error"]["code"], 404);
        assert_eq!(json["objects"][1]["error"]["code"], 422);

        let bad_cases: &[(&str, &[u8])] = &[
            (
                r#"{"operation":"nonsense","objects":[]}"#,
                b"HTTP/1.1 400 Bad Request",
            ),
            (
                r#"{"operation":"download","transfers":["other"],"objects":[]}"#,
                b"HTTP/1.1 422 Unprocessable Entity",
            ),
            (
                r#"{"operation":"download","hash_algo":"sha1","objects":[]}"#,
                b"HTTP/1.1 409 Conflict",
            ),
            ("not JSON", b"HTTP/1.1 400 Bad Request"),
        ];
        for &(json, status) in bad_cases {
            let response = exchange_in_store(&batch_request(json), &directory.0);
            let (head, body) = response_parts(&response);
            assert!(head.starts_with(status));
            assert!(head
                .windows(LFS_JSON.len())
                .any(|part| part == LFS_JSON.as_bytes()));
            let json: serde_json::Value = serde_json::from_slice(body).unwrap();
            assert!(json["message"].as_str().is_some());
            assert!(json.get("objects").is_none());
        }

        let wrong_type = b"POST /objects/batch HTTP/1.1\r\nAccept: application/vnd.git-lfs+json\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\n{}";
        let response = exchange_in_store(wrong_type, &directory.0);
        assert!(response.starts_with(b"HTTP/1.1 415 Unsupported Media Type\r\n"));
    }
}
