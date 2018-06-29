use std::fmt;
use super::Error;
use super::StatusCode;
use super::Version;

#[derive(Debug)]
pub struct ResponseStatus {
    pub version: Version,
    pub status: StatusCode,
}

impl ResponseStatus {
    pub fn new(status: StatusCode) -> ResponseStatus {
        let version = Version::new(1, 1);
        ResponseStatus{ version, status }
    }

    pub fn from(line: String) -> Result<ResponseStatus, Error> {
        // status-line = HTTP-version SP status-code SP reason-phrase CRLF
        // Split by space.
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 3 {
            return Error::err("Bad request");
        }
        // Parse method, target and version.
        let version = Version::from(parts[0])?;
        let status: u16 = parts[1].parse()?;
        let status = StatusCode::from(status).ok_or(Error::new("Bad status"))?;

        Ok(ResponseStatus{ version, status })
    }
}

impl fmt::Display for ResponseStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{} {} {}", self.version, self.status.code(),
                       self.status.phrase()))
    }
}
