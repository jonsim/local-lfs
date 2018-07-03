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
            return Error::err("Invalid request");
        }
        // Parse method, target and version.
        let method = Method::from(parts[0])?;
        let target = String::from(parts[1]);
        if target.is_empty() {
            return Error::err("Invalid target");
        }
        let version = Version::from(parts[2])?;

        Ok(RequestStatus{ version, method, target })
    }
}

impl fmt::Display for RequestStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{} {} {}", self.method, self.target, self.version))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::*;

    fn assert_status_equals(version: &Version, method: &Method, target: &String,
            actual: &RequestStatus) {
        assert_eq!(version, &actual.version);
        assert_eq!(method, &actual.method);
        assert_eq!(target, &actual.target);
    }

    #[test]
    fn new() {
        let target = String::from("/hello");
        let status = RequestStatus::new(Method::GET, target.clone());
        assert_status_equals(&Version::new(1, 1), &Method::GET, &target,
            &status);
    }

    #[test]
    fn from_valid_string() {
        // Test an easy one.
        let input = String::from("GET /foo/bar HTTP/1.2");
        let status = RequestStatus::from(input).unwrap();
        assert_status_equals(&Version::new(1,2), &Method::GET,
            &String::from("/foo/bar"), &status);

        // Test a slightly harder one.
        let input = String::from("CONNECT / HTTP/0.0");
        let status = RequestStatus::from(input).unwrap();
        assert_status_equals(&Version::new(0,0), &Method::CONNECT,
            &String::from("/"), &status);
    }

    #[test]
    fn from_invalid_empty_string() {
        // Empty string.
        let status = RequestStatus::from(String::from(""));
        assert_parse_error("HTTP parsing error: Invalid request", status);

        // String of empty parameters.
        let status = RequestStatus::from(String::from("  "));
        assert_parse_error("HTTP parsing error: Invalid method", status);
    }

    #[test]
    fn from_invalid_too_few_args() {
        let status = RequestStatus::from(String::from("CONNECT /"));
        assert_parse_error("HTTP parsing error: Invalid request", status);
    }

    #[test]
    fn from_invalid_too_many_args() {
        let status = RequestStatus::from(String::from("HEAD /a /b HTTP/1.1"));
        assert_parse_error("HTTP parsing error: Invalid request", status);
    }

    #[test]
    fn from_invalid_method_arg() {
        // Non-existant method.
        let status = RequestStatus::from(String::from("FOO /foo HTTP/1.1"));
        assert_parse_error("HTTP parsing error: Invalid method", status);

        // Empty method.
        let status = RequestStatus::from(String::from(" /foo HTTP/1.1"));
        assert_parse_error("HTTP parsing error: Invalid method", status);
    }

    #[test]
    fn from_invalid_target_arg() {
        // Empty target.
        let status = RequestStatus::from(String::from("GET  HTTP/1.1"));
        assert_parse_error("HTTP parsing error: Invalid target", status);
    }

    #[test]
    fn from_invalid_version_arg() {
        // Non-numeric version.
        let status = RequestStatus::from(String::from("GET /foo HTTP/a.1"));
        assert_parse_error("HTTP parsing error: Invalid version", status);

        // Empty version.
        let status = RequestStatus::from(String::from("GET /foo "));
        assert_parse_error("HTTP parsing error: Invalid version", status);
    }

    #[test]
    fn display() {
        let input = "OPTIONS * HTTP/1.1";
        let status = RequestStatus::from(String::from(input)).unwrap();
        assert_eq!(input, format!("{}", status));
    }
}
