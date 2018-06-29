use std::fmt;
use super::Field;
use super::ResponseStatus;
use super::StatusCode;

#[derive(Debug)]
pub struct Response {
    line: ResponseStatus,
    headers: Vec<Field>,
}

impl Response {
    pub fn from(status: StatusCode) -> Response {
        let line = ResponseStatus::new(status);
        let headers = Vec::new();
        Response{ line, headers }
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut out = String::new();
        out.push_str(&format!("{}\r\n", self.line));
        for boogle in &self.headers {
            out.push_str(&format!("{}\r\n", boogle));
        }
        f.pad(&out)
    }
}
