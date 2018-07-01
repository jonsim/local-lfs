mod error;
mod method;
mod version;
mod status_code;
mod field;
mod request_status;
mod response_status;
mod request;
mod response;
mod body;

pub use self::error::ParseError as Error;
pub use self::method::Method;
pub use self::version::Version;
pub use self::status_code::StatusCode;
pub use self::field::Field;
use self::request_status::RequestStatus;
use self::response_status::ResponseStatus;
pub use self::request::Request;
pub use self::response::Response;
pub use self::body::Body;

enum Status {
    Request(RequestStatus),
    Response(ResponseStatus),
}

pub struct MessageBuilder {
    status: Status,
    fields: Vec<Field>,
    body: String,
}

impl MessageBuilder {
    pub fn request(method: Method, target: String) -> MessageBuilder {
        let status = Status::Request(RequestStatus::new(method, target));
        MessageBuilder{ status, fields: Vec::new(), body: String::new() }
    }

    pub fn response(code: StatusCode) -> MessageBuilder {
        let status = Status::Response(ResponseStatus::new(code));
        MessageBuilder{ status, fields: Vec::new(), body: String::new() }
    }

    pub fn add_field(&mut self, field: Field) -> &mut Self {
        self.fields.push(field);
        self
    }

    pub fn add_body(&mut self, body: String) -> &mut Self {
        self.body = body;
        self
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let body = Body::from(self.body);
        let message = match self.status {
            Status::Request(status) => {
                let head = Request::new(status, self.fields);
                format!("{}\r\n{}", head, body)
            },
            Status::Response(status) => {
                let head = Response::new(status, self.fields);
                format!("{}\r\n{}", head, body)
            },
        };
        message.into_bytes()
    }
}


#[cfg(test)]
mod tests {
    use std::cmp;
    use std::fmt;
    use std::io::{Read, BufRead};
    use std::io::Result as IoResult;
    use super::Error;

    pub struct StringReader {
        content: String,
        pos: usize,
    }

    impl StringReader {
        pub fn new<'a>(content: &'a str) -> StringReader {
            StringReader{ content: String::from(content), pos: 0 }
        }
    }

    impl Read for StringReader {
        fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
            let len = cmp::min(buf.len(), self.content.len() - self.pos);
            let end = self.pos + len;
            &buf[..len].clone_from_slice(&self.content.as_bytes()[self.pos..end]);
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


    pub fn assert_parse_error<T: fmt::Debug>(message: &str, result: Result<T, Error>) {
        assert!(result.is_err());
        let description = format!("{}", result.unwrap_err());
        assert_eq!(message, description);
    }
}
