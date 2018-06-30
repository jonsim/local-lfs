use std::fmt;
use super::Field;
use super::ResponseStatus;

#[derive(Debug)]
pub struct Response {
    line: ResponseStatus,
    fields: Vec<Field>,
}

impl Response {
    pub fn new(line: ResponseStatus, fields: Vec<Field>) -> Response {
        Response{ line, fields }
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut out = String::new();
        out.push_str(&format!("{}\r\n", self.line));
        for boogle in &self.fields {
            out.push_str(&format!("{}\r\n", boogle));
        }
        f.pad(&out)
    }
}
