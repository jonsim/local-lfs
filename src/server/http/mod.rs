pub mod status_code;

use std::fmt;
use std::error::Error as StdError;
use std::io::Error as IoError;
use std::num::ParseIntError;
use self::status_code::StatusCode;


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
pub struct RequestHeader {
    line: RequestHeaderStatusLine,
    headers: Vec<HeaderField>,
}

#[derive(Debug)]
struct ResponseHeader {
    line: ResponseHeaderStatusLine,
    headers: Vec<HeaderField>,
}

#[derive(Debug, PartialEq)]
pub struct Version {
    major: u8,
    minor: u8,
}

#[derive(Debug, PartialEq)]
pub enum Method {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    TRACE,
    OPTIONS,
    CONNECT,
    PATCH,
}

#[derive(Debug)]
struct RequestHeaderStatusLine {
    version: Version,
    method: Method,
    target: String,
}

#[derive(Debug)]
struct ResponseHeaderStatusLine {
    version: Version,
    status: StatusCode,
}

#[derive(Debug)]
struct HeaderField {
    name:  String,
    value: String,
}

#[derive(Debug)]
struct Body {
    content: String,
}

#[derive(Debug)]
pub struct Response {
    header: ResponseHeader,
    body: Body,
}

impl<'a> RequestHeader {
    pub fn parse<Iter>(lines: &'a mut Iter) -> Result<RequestHeader, ParseError>
    where
        Iter: Iterator<Item = Result<String, IoError>>,
    {
        let first_line = lines.next()
            .ok_or(ParseError::new("Unexpected end of stream"))??;
        let line = RequestHeaderStatusLine::from(first_line)?;
        let mut headers = Vec::new();
        loop {
            let iline = lines.next()
                .ok_or(ParseError::new("Unexpected end of stream"))??;
            if iline.is_empty() {
                break;  // header finished.
            }
            let header = HeaderField::from(iline)?;
            headers.push(header);
        }

        Ok(RequestHeader{ line, headers })
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
impl fmt::Display for RequestHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut out = String::new();
        out.push_str(&format!("{}\r\n", self.line));
        for boogle in &self.headers {
            out.push_str(&format!("{}\r\n", boogle));
        }
        f.pad(&out)
    }
}


impl Body {
    pub fn content_length(&self) -> usize {
        self.content.len()
    }
}
impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{}", self.content))
    }
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

impl ResponseHeader {
    pub fn build(status: StatusCode) -> ResponseHeader {
        let line = ResponseHeaderStatusLine::new(status);
        let headers = Vec::new();
        ResponseHeader{ line, headers }
    }
}
impl fmt::Display for ResponseHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut out = String::new();
        out.push_str(&format!("{}\r\n", self.line));
        for boogle in &self.headers {
            out.push_str(&format!("{}\r\n", boogle));
        }
        f.pad(&out)
    }
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

    pub fn major(&self) -> u8 {
        self.major
    }

    pub fn minor(&self) -> u8 {
        self.minor
    }
}
impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("HTTP/{}.{}", self.major, self.minor))
    }
}

impl Method {
    fn from(version_str: &str) -> Result<Method, ParseError> {
        match version_str {
            "GET"     => Ok(Method::GET),
            "HEAD"    => Ok(Method::HEAD),
            "POST"    => Ok(Method::POST),
            "PUT"     => Ok(Method::PUT),
            "DELETE"  => Ok(Method::DELETE),
            "TRACE"   => Ok(Method::TRACE),
            "OPTIONS" => Ok(Method::OPTIONS),
            "CONNECT" => Ok(Method::CONNECT),
            "PATCH"   => Ok(Method::PATCH),
            _ => ParseError::err("Invalid method"),
        }
    }
}
impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl RequestHeaderStatusLine {
    fn from(line: String) -> Result<RequestHeaderStatusLine, ParseError> {
        // request-line = method SP request-target SP HTTP-version CRLF
        // Split by space.
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 3 {
            return ParseError::err("Bad request");
        }
        // Parse method, target and version.
        let method = Method::from(parts[0])?;
        let target = String::from(parts[1]);
        let version = Version::from(parts[2])?;

        Ok(RequestHeaderStatusLine{ version, method, target })
    }
}
impl fmt::Display for RequestHeaderStatusLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{} {} {}", self.method, self.target, self.version))
    }
}

impl ResponseHeaderStatusLine {
    fn new(status: StatusCode) -> ResponseHeaderStatusLine {
        let version = Version{ major: 1, minor: 1 };
        ResponseHeaderStatusLine{ version, status }
    }

    fn from(line: String) -> Result<ResponseHeaderStatusLine, ParseError> {
        // status-line = HTTP-version SP status-code SP reason-phrase CRLF
        // Split by space.
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 3 {
            return ParseError::err("Bad request");
        }
        // Parse method, target and version.
        let version = Version::from(parts[0])?;
        let status: u16 = parts[1].parse()?;
        let status = StatusCode::from(status).ok_or(ParseError::new("Bad status"))?;

        Ok(ResponseHeaderStatusLine{ version, status })
    }
}
impl fmt::Display for ResponseHeaderStatusLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{} {} {}", self.version, self.status.code(),
                       self.status.phrase()))
    }
}

impl HeaderField {
    fn new(name: String, value: String) -> HeaderField {
        HeaderField{ name, value }
    }

    fn ContentLength(length: usize) -> HeaderField {
        let name = String::from("Content-Length");
        let value = format!("{}", length);
        HeaderField{ name, value }
    }

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
