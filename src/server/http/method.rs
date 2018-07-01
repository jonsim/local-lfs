use std::fmt;
use super::Error;

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

impl Method {
    pub fn from(method: &str) -> Result<Method, Error> {
        match method {
            "GET"     => Ok(Method::GET),
            "HEAD"    => Ok(Method::HEAD),
            "POST"    => Ok(Method::POST),
            "PUT"     => Ok(Method::PUT),
            "DELETE"  => Ok(Method::DELETE),
            "TRACE"   => Ok(Method::TRACE),
            "OPTIONS" => Ok(Method::OPTIONS),
            "CONNECT" => Ok(Method::CONNECT),
            "PATCH"   => Ok(Method::PATCH),
            _ => Error::err("Invalid method"),
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::*;

    #[test]
    fn from_valid_string() {
        let names = ["CONNECT", "DELETE", "GET",
                     "HEAD", "OPTIONS", "PATCH",
                     "POST", "PUT", "TRACE"];
        let mut enums = [Method::CONNECT, Method::DELETE, Method::GET,
                         Method::HEAD, Method::OPTIONS, Method::PATCH,
                         Method::POST, Method::PUT, Method::TRACE];
        assert_eq!(names.len(), enums.len());
        for (value, expected) in names.iter().zip(enums.iter_mut()) {
            let mut actual = Method::from(value).unwrap();
            assert_eq!(expected, &actual);
            assert_eq!(value, &format!("{}", actual));
        }
    }

    #[test]
    fn from_invalid_string() {
        // Test empty string.
        let result = Method::from("");
        assert_parse_error("HTTP parsing error: Invalid method", result);

        // Test lower case valid method.
        let result = Method::from("Get");
        assert_parse_error("HTTP parsing error: Invalid method", result);
    }
}