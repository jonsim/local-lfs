use std::fmt;
use std::error::Error as StdError;
use std::io::Error as IoError;
use std::num::ParseIntError;

#[derive(Debug)]
pub struct ParseError {
    description: &'static str,
}
impl ParseError {
    fn new(description: &'static str) -> ParseError {
        ParseError{ description }
    }
    fn err<T>(description: &'static str) -> Result<T, ParseError> {
        Err(ParseError::new(description))
    }
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("HTTP parsing error: {}", self.description))
    }
}
impl StdError for ParseError {
    fn description(&self) -> &str {
        self.description
    }
}
impl From<ParseIntError> for ParseError {
    fn from(_: ParseIntError) -> ParseError {
        ParseError::new("cannot parse integer")
    }
}
impl From<IoError> for ParseError {
    fn from(_: IoError) -> ParseError {
        ParseError::new("failed to read from connection")
    }
}


#[derive(Debug)]
pub struct Request {
    request_line: RequestLine,
    headers: Vec<HeaderField>,
}

#[derive(Debug)]
struct Response {

}

#[derive(Debug)]
struct Version {
    major: u8,
    minor: u8,
}

#[derive(Debug)]
struct RequestLine {
    version: Version,
    method: String,
    target: String,
}

#[derive(Debug)]
struct StatusLine {
    version: Version,
    status: u16,
    reason: String,
}

#[derive(Debug)]
struct HeaderField {
    name:  String,
    value: String,
}

impl<'a> Request {
    pub fn parse<Iter>(lines: &'a mut Iter) -> Result<Request, ParseError>
    where
        Iter: Iterator<Item = Result<String, IoError>>,
    {
        let first_line = lines.next()
            .ok_or(ParseError::new("Unexpected end of stream"))??;
        let request_line = RequestLine::from(first_line)?;
        let mut headers: Vec<HeaderField> = Vec::new();
        loop {
            let line = lines.next()
                .ok_or(ParseError::new("Unexpected end of stream"))??;
            if line.is_empty() {
                break;  // header finished.
            }
            let header = HeaderField::from(line)?;
            headers.push(header);
        }

        Ok(Request{ request_line, headers })
    }
}

impl Version {
    fn from(version_str: String) -> Result<Version, ParseError> {
        // version = HTTP-M.m
        // Split into bytes.
        let version: Vec<u8> = version_str.bytes().collect();
        if version.len() != 8 {
            return ParseError::err("Bad version");
        }
        // Parse major and minor versions.
        let major: u8 = version[5] - b'0';
        let minor: u8 = version[7] - b'0';
        if major > 9 || minor > 9 {
            return ParseError::err("Bad version");
        }

        Ok(Version{ major, minor })
    }
}
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("HTTP-{}.{}", self.major, self.minor))
    }
}

impl RequestLine {
    fn from(line: String) -> Result<RequestLine, ParseError> {
        // request-line = method SP request-target SP HTTP-version CRLF
        // Split by space.
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 3 {
            return ParseError::err("Bad request");
        }
        // Parse method, target and version.
        let method = String::from(parts[0]);
        let target = String::from(parts[1]);
        let version = Version::from(String::from(parts[2]))?;

        Ok(RequestLine{ version, method, target })
    }
}
impl fmt::Display for RequestLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{} {} {}\r\n", self.method, self.target, self.version))
    }
}

impl StatusLine {
    fn from(line: String) -> Result<StatusLine, ParseError> {
        // status-line = HTTP-version SP status-code SP reason-phrase CRLF
        // Split by space.
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 3 {
            return ParseError::err("Bad request");
        }
        // Parse method, target and version.
        let version = Version::from(String::from(parts[0]))?;
        let status: u16 = parts[1].parse()?;
        let reason = String::from(parts[2]);

        Ok(StatusLine{ version, status, reason })
    }
}
impl fmt::Display for StatusLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{} {} {}\r\n", self.version, self.status, self.reason))
    }
}

impl HeaderField {
    fn from(line: String) -> Result<HeaderField, ParseError> {
        // header-field   = field-name ":" OWS field-value OWS
        // field-value    = *( field-content / obs-fold )
        // field-content  = field-vchar [ 1*( SP / HTAB ) field-vchar ]
        // field-vchar    = VCHAR / obs-text
        // obs-fold       = CRLF 1*( SP / HTAB )
        //                ; obsolete line folding (see Section 3.2.4)
        // Split by the first colon separator.
        let sep = line.find(':');
        if sep.is_none() {
            return ParseError::err("Bad HTTP header");
        }
        // Parse name. Names must not contain whitespace.
        let name = String::from(&line[..sep.unwrap()]);
        if name.find(char::is_whitespace).is_some() {
            return ParseError::err("Bad HTTP header");
        }
        // Parse value. Values must have leading/trailing whitespace removed.
        // Line folding unsupported.
        let value = String::from(line[sep.unwrap()+1..].trim());
        if value.find('\n').is_some() {
            return ParseError::err("Bad HTTP header");
        }

        Ok(HeaderField{ name, value })
    }
}
impl fmt::Display for HeaderField {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{}: {}", self.name, self.value))
    }
}

