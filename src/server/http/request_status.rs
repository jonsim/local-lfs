use std::fmt;
use super::Error;
use super::Method;
use super::Version;

#[derive(Debug)]
pub struct RequestStatus {
    pub version: Version,
    pub method: Method,
    pub target: String,
}

impl RequestStatus {
    pub fn new(method: Method, target: String) -> RequestStatus {
        let version = Version::new(1, 1);
        RequestStatus{ version, method, target }
    }

    pub fn from(line: String) -> Result<RequestStatus, Error> {
        // request-line = method SP request-target SP HTTP-version CRLF
        // Split by space.
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 3 {
            return Error::err("Bad request");
        }
        // Parse method, target and version.
        let method = Method::from(parts[0])?;
        let target = String::from(parts[1]);
        let version = Version::from(parts[2])?;

        Ok(RequestStatus{ version, method, target })
    }
}

impl fmt::Display for RequestStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{} {} {}", self.method, self.target, self.version))
    }
}
