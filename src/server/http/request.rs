use std::fmt;
use std::io::Error as IoError;
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

impl<'a> Request {
    pub fn parse<Iter>(lines: &'a mut Iter) -> Result<Request, Error>
    where
        Iter: Iterator<Item = Result<String, IoError>>,
    {
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
            let header = Field::from(iline)?;
            fields.push(header);
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
