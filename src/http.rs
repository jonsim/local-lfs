use std::fmt;

#[derive(Debug)]
struct Request {
    line: RequestLine,
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
    name: String,
    value: String,
}

impl Version {
    fn from(version_str: &str) -> Result<Version, &'static str> {
        // version = HTTP-M.m
        // Split into bytes.
        let version: Vec<u8> = version_str.bytes().collect();
        if version.len() != 8 {
            return Err("Bad HTTP version");
        }
        // Parse major and minor versions.
        let major: u8 = version[5] - b'0';
        let minor: u8 = version[7] - b'0';
        if major > 9 || minor > 9 {
            return Err("Bad HTTP version");
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
    fn from(line: &str) -> Result<RequestLine, &'static str> {
        // request-line = method SP request-target SP HTTP-version CRLF
        // Split by space.
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() != 3 {
            return Err("Bad HTTP request");
        }
        // Parse method, target and version.
        let method = String::from(parts[0]);
        let target = String::from(parts[1]);
        let version = Version::from(parts[2])?;

        Ok(RequestLine{ version, method, target })
    }
}
impl fmt::Display for RequestLine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{} {} {}\r\n", self.method, self.target, self.version))
    }
}

impl HeaderField {
    fn from(line: &str) -> Result<HeaderField, &'static str> {
        // header-field   = field-name ":" OWS field-value OWS
        // field-value    = *( field-content / obs-fold )
        // field-content  = field-vchar [ 1*( SP / HTAB ) field-vchar ]
        // field-vchar    = VCHAR / obs-text
        // obs-fold       = CRLF 1*( SP / HTAB )
        //                ; obsolete line folding (see Section 3.2.4)
        // Split by the first colon separator.
        let sep = line.find(':');
        if sep.is_none() {
            return Err("Bad HTTP header");
        }
        // Parse name. Names must not contain whitespace.
        let name = String::from(&line[..sep.unwrap()]);
        if name.find(char::is_whitespace).is_some() {
            return Err("Bad HTTP header");
        }
        // Parse value. Values must have leading/trailing whitespace removed.
        // Line folding unsupported.
        let value = String::from(line[sep.unwrap()+1..].trim());
        if value.find('\n').is_some() {
            return Err("Bad HTTP header");
        }

        Ok(HeaderField{ name, value })
    }
}
impl fmt::Display for HeaderField {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{}: {}", self.name, self.value))
    }
}