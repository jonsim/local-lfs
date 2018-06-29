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
