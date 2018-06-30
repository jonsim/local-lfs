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


/*
#[derive(Debug)]
pub struct Response {
    header: ResponseHeader,
    body: Body,
}



impl Response {
    pub fn build(status: StatusCode, content: String) -> Response {
        let body = Body { content };
        let content_length = body.content_length();
        let mut header = ResponseHeader::build(status);
        header.headers.push(HeaderField::ContentLength(content_length));
        Response{ header, body }
    }
}
impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{}\r\n{}", self.header, self.body))
    }
}
*/
