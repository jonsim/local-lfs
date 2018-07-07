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

#[derive(Debug, PartialEq)]
enum Status {
    Request(RequestStatus),
    Response(ResponseStatus),
}

#[derive(Debug, PartialEq)]
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

    pub fn add_field2(&mut self, name: &str, value: &str) -> &mut Self {
        self.fields.push(Field::new(String::from(name), String::from(value)));
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
                format!("{}{}", head, body)
            },
            Status::Response(status) => {
                let head = Response::new(status, self.fields);
                format!("{}{}", head, body)
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
    use super::*;

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


    const REQ_METHOD: Method = Method::GET;
    const REQ_TARGET: &'static str = "/foo/bar";
    const RSP_CODE: StatusCode = StatusCode::ImATeapot;
    const FIELD_N1: &'static str = "david";
    const FIELD_V1: &'static str = "suchet";
    const FIELD_N2: &'static str = "hello";
    const FIELD_V2: &'static str = "world";
    const BODY:     &'static str = "ze little grey cells";
    const REQUEST_ENCODING_2FB: &'static str = "\
        GET /foo/bar HTTP/1.1\r\n\
        david: suchet\r\n\
        hello: world\r\n\
        \r\n\
        ze little grey cells";
    const REQUEST_ENCODING_1FB: &'static str = "\
        GET /foo/bar HTTP/1.1\r\n\
        david: suchet\r\n\
        \r\n\
        ze little grey cells";
    const REQUEST_ENCODING_0FB: &'static str = "\
        GET /foo/bar HTTP/1.1\r\n\
        \r\n\
        ze little grey cells";
    const REQUEST_ENCODING_0F: &'static str = "\
        GET /foo/bar HTTP/1.1\r\n\
        \r\n";
    const RESPONSE_ENCODING_2FB: &'static str = "\
        HTTP/1.1 418 I'm a teapot\r\n\
        david: suchet\r\n\
        hello: world\r\n\
        \r\n\
        ze little grey cells";
    const RESPONSE_ENCODING_1FB: &'static str = "\
        HTTP/1.1 418 I'm a teapot\r\n\
        david: suchet\r\n\
        \r\n\
        ze little grey cells";
    const RESPONSE_ENCODING_0FB: &'static str = "\
        HTTP/1.1 418 I'm a teapot\r\n\
        \r\n\
        ze little grey cells";
    const RESPONSE_ENCODING_0F: &'static str = "\
        HTTP/1.1 418 I'm a teapot\r\n\
        \r\n";

    #[test]
    fn request() {
        let mut builder = MessageBuilder::request(REQ_METHOD, String::from(REQ_TARGET));
        builder.add_field2(FIELD_N1, FIELD_V1)
               .add_field2(FIELD_N2, FIELD_V2)
               .add_body(String::from(BODY));
        assert_eq!(REQUEST_ENCODING_2FB.as_bytes(), builder.into_bytes().as_slice());

        let mut builder = MessageBuilder::request(REQ_METHOD, String::from(REQ_TARGET));
        builder.add_field2(FIELD_N1, FIELD_V1)
               .add_body(String::from(BODY));
        assert_eq!(REQUEST_ENCODING_1FB.as_bytes(), builder.into_bytes().as_slice());

        let mut builder = MessageBuilder::request(REQ_METHOD, String::from(REQ_TARGET));
        builder.add_body(String::from(BODY));
        assert_eq!(REQUEST_ENCODING_0FB.as_bytes(), builder.into_bytes().as_slice());

        let builder = MessageBuilder::request(REQ_METHOD, String::from(REQ_TARGET));
        assert_eq!(REQUEST_ENCODING_0F.as_bytes(), builder.into_bytes().as_slice());
    }

    #[test]
    fn response() {
        let mut builder = MessageBuilder::response(RSP_CODE);
        builder.add_field2(FIELD_N1, FIELD_V1)
               .add_field2(FIELD_N2, FIELD_V2)
               .add_body(String::from(BODY));
        assert_eq!(RESPONSE_ENCODING_2FB.as_bytes(), builder.into_bytes().as_slice());

        let mut builder = MessageBuilder::response(RSP_CODE);
        builder.add_field2(FIELD_N1, FIELD_V1)
               .add_body(String::from(BODY));
        assert_eq!(RESPONSE_ENCODING_1FB.as_bytes(), builder.into_bytes().as_slice());

        let mut builder = MessageBuilder::response(RSP_CODE);
        builder.add_body(String::from(BODY));
        assert_eq!(RESPONSE_ENCODING_0FB.as_bytes(), builder.into_bytes().as_slice());

        let builder = MessageBuilder::response(RSP_CODE);
        assert_eq!(RESPONSE_ENCODING_0F.as_bytes(), builder.into_bytes().as_slice());
    }

    #[test]
    fn add_fields_equivalent() {
        let mut builder1 = MessageBuilder::request(REQ_METHOD, String::from(REQ_TARGET));
        let mut builder2 = MessageBuilder::request(REQ_METHOD, String::from(REQ_TARGET));
        builder1.add_field(Field::new(String::from(FIELD_N1), String::from(FIELD_V1)));
        builder2.add_field2(FIELD_N1, FIELD_V1);
        assert_eq!(builder1, builder2);
    }
}
