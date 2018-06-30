use std::fmt;
use std::io::BufRead;
use super::Error;
use super::Field;
use super::Method;
use super::RequestStatus;
use super::Version;

#[derive(Debug)]
pub struct Request {
    line: RequestStatus,
    fields: Vec<Field>,
}

impl Request {
    pub fn new(line: RequestStatus, fields: Vec<Field>) -> Request {
        Request{ line, fields }
    }

    pub fn parse<B: BufRead>(reader: &mut B) -> Result<Request, Error>
    {
        let mut lines = reader.lines();
        let first_line = lines.next()
            .ok_or(Error::new("Unexpected end of stream"))??;
        let line = RequestStatus::from(first_line)?;
        let mut fields = Vec::new();
        loop {
            let iline = lines.next()
                .ok_or(Error::new("Unexpected end of stream"))??;
            if iline.is_empty() {
                break;  // header finished.
            }
            let field = Field::from(iline)?;
            fields.push(field);
        }

        Ok(Request{ line, fields })
    }

    pub fn version(&self) -> &Version {
        &self.line.version
    }

    pub fn method(&self) -> &Method {
        &self.line.method
    }

    pub fn target(&self) -> &str {
        &self.line.target
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut out = String::new();
        out.push_str(&format!("{}\r\n", self.line));
        for boogle in &self.fields {
            out.push_str(&format!("{}\r\n", boogle));
        }
        f.pad(&out)
    }
}
