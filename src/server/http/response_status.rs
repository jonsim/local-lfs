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
        let version = Version::new(1, 1).unwrap();
        ResponseStatus{ version, status }
    }

    pub fn from(line: String) -> Result<ResponseStatus, Error> {
        // status-line = HTTP-version SP status-code SP reason-phrase CRLF
        // Split by space.
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() != 3 {
            return Error::err("Invalid response");
        }
        // Parse method, target and version.
        let version = Version::from(parts[0])?;
        let status: u16 = parts[1].parse().map_err(|_| Error::new("Invalid status"))?;
        let status = StatusCode::from(status).ok_or(Error::new("Invalid status"))?;

        Ok(ResponseStatus{ version, status })
    }
}

impl fmt::Display for ResponseStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{} {} {}", self.version, self.status.code(),
                       self.status.phrase()))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::*;

    fn assert_status_equals(version: &Version, status: &StatusCode,
            actual: &ResponseStatus) {
        assert_eq!(version, &actual.version);
        assert_eq!(status, &actual.status);
    }

    #[test]
    fn new() {
        let status = ResponseStatus::new(StatusCode::Ok);
        assert_status_equals(&Version::new(1, 1).unwrap(), &StatusCode::Ok,
            &status);
    }

    #[test]
    fn from_valid_string() {
        // Test an easy one.
        let input = String::from("HTTP/1.1 200 OK");
        let status = ResponseStatus::from(input).unwrap();
        assert_status_equals(&Version::new(1,1).unwrap(), &StatusCode::Ok,
            &status);

        // Test a slightly harder one.
        let input = String::from("HTTP/0.2 418 I'm a teapot");
        let status = ResponseStatus::from(input).unwrap();
        assert_status_equals(&Version::new(0,2).unwrap(), &StatusCode::ImATeapot,
            &status);
    }

    #[test]
    fn from_invalid_empty_string() {
        // Empty string.
        let status = ResponseStatus::from(String::from(""));
        assert_parse_error("HTTP parsing error: Invalid response", status);

        // String of empty parameters.
        let status = ResponseStatus::from(String::from("  "));
        assert_parse_error("HTTP parsing error: Invalid version", status);
    }

    #[test]
    fn from_invalid_too_few_args() {
        let status = ResponseStatus::from(String::from("HTTP/1.1 200"));
        assert_parse_error("HTTP parsing error: Invalid response", status);
    }

    #[test]
    fn from_invalid_version_arg() {
        // Non-numeric version.
        let status = ResponseStatus::from(String::from("HTTP/1.I 200 OK"));
        assert_parse_error("HTTP parsing error: Invalid version", status);

        // Empty version.
        let status = ResponseStatus::from(String::from(" 200 OK"));
        assert_parse_error("HTTP parsing error: Invalid version", status);
    }

    #[test]
    fn from_invalid_statuscode_arg() {
        // Too-long numeric status code.
        let status = ResponseStatus::from(String::from("HTTP/1.1 1234 OK"));
        assert_parse_error("HTTP parsing error: Invalid status", status);

        // Too-short numeric status code.
        let status = ResponseStatus::from(String::from("HTTP/1.1 12 OK"));
        assert_parse_error("HTTP parsing error: Invalid status", status);

        // Invalid numeric status code.
        let status = ResponseStatus::from(String::from("HTTP/1.1 420 OK"));
        assert_parse_error("HTTP parsing error: Invalid status", status);

        // Non-numeric status code.
        let status = ResponseStatus::from(String::from("HTTP/1.1 #420 OK"));
        assert_parse_error("HTTP parsing error: Invalid status", status);

        // Empty status code.
        let status = ResponseStatus::from(String::from("HTTP/1.1  OK"));
        assert_parse_error("HTTP parsing error: Invalid status", status);
    }

    #[test]
    fn from_ignores_phrase_arg() {
        // Empty phrase.
        let input = String::from("HTTP/1.1 200 ");
        let status = ResponseStatus::from(input).unwrap();
        assert_status_equals(&Version::new(1,1).unwrap(), &StatusCode::Ok,
            &status);

        // Mismatched phrase and status code.
        let input = String::from("HTTP/1.1 404 Payload Too Large");
        let status = ResponseStatus::from(input).unwrap();
        assert_status_equals(&Version::new(1,1).unwrap(), &StatusCode::NotFound,
            &status);

        // Totally bogus phrase.
        let input = String::from("HTTP/1.1 410 420");
        let status = ResponseStatus::from(input).unwrap();
        assert_status_equals(&Version::new(1,1).unwrap(), &StatusCode::Gone,
            &status);
    }

    #[test]
    fn display() {
        let input = "HTTP/1.2 413 Payload Too Large";
        let status = ResponseStatus::from(String::from(input)).unwrap();
        assert_eq!(input, format!("{}", status));
    }
}
