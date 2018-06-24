use std::fmt;
use std::error::Error;
use std::num::ParseIntError;

#[derive(Debug)]
struct ParseError {
    description: &'static str,
}
impl ParseError {
    fn err<T>(description: &'static str) -> Result<T, ParseError> {
        Err(ParseError{ description })
    }
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("HTTP parsing error: {}", self.description))
    }
}
impl Error for ParseError {
    fn description(&self) -> &str {
        self.description
    }
}
impl From<ParseIntError> for ParseError {
    fn from(_: ParseIntError) -> ParseError {
        ParseError{ description: "cannot parse integer" }
    }
}


#[derive(Debug)]
struct Request<'a> {
    line: RequestLine<'a>,
    headers: Vec<HeaderField<'a>>,
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
struct RequestLine<'a> {
    version: Version,
    method: &'a str,
    target: &'a str,
}

#[derive(Debug)]
struct StatusLine<'a> {
    version: Version,
    status: u16,
    reason: &'a str,
}

#[derive(Debug)]
struct HeaderField<'a> {
    name:  &'a str,
    value: &'a str,
}

impl Version {
    fn from(version_str: &str) -> Result<Version, ParseError> {
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

impl<'a> RequestLine<'a> {
    fn from(line: &'a str) -> Result<RequestLine<'a>, ParseError> {
        // request-line = method SP request-target SP HTTP-version CRLF
        // Split by space.
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 3 {
            return ParseError::err("Bad request");
        }
        // Parse method, target and version.
        let method = parts[0];
        let target = parts[1];
        let version = Version::from(parts[2])?;

        Ok(RequestLine{ version, method, target })
    }
}
impl<'a> fmt::Display for RequestLine<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{} {} {}\r\n", self.method, self.target, self.version))
    }
}

impl<'a> StatusLine<'a> {
    fn from(line: &'a str) -> Result<StatusLine<'a>, ParseError> {
        // status-line = HTTP-version SP status-code SP reason-phrase CRLF
        // Split by space.
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 3 {
            return ParseError::err("Bad request");
        }
        // Parse method, target and version.
        let version = Version::from(parts[0])?;
        let status: u16 = parts[1].parse()?;
        let reason = parts[2];

        Ok(StatusLine{ version, status, reason })
    }
}

impl<'a> HeaderField<'a> {
    fn from(line: &'a str) -> Result<HeaderField<'a>, ParseError> {
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
        let name = &line[..sep.unwrap()];
        if name.find(char::is_whitespace).is_some() {
            return ParseError::err("Bad HTTP header");
        }
        // Parse value. Values must have leading/trailing whitespace removed.
        // Line folding unsupported.
        let value = line[sep.unwrap()+1..].trim();
        if value.find('\n').is_some() {
            return ParseError::err("Bad HTTP header");
        }

        Ok(HeaderField{ name, value })
    }
}
impl<'a> fmt::Display for HeaderField<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{}: {}", self.name, self.value))
    }
}

