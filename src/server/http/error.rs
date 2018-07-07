use std::fmt;
use std::error::Error as StdError;
use std::io::Error as IoError;

#[derive(Debug)]
pub struct ParseError {
    description: &'static str,
}

impl ParseError {
    pub fn new(description: &'static str) -> ParseError {
        ParseError{ description }
    }
    pub fn err<T>(description: &'static str) -> Result<T, ParseError> {
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

impl From<IoError> for ParseError {
    fn from(_: IoError) -> ParseError {
        ParseError::new("Failed to read from connection")
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    fn assert_error_eq(desc: &str, error: &ParseError) {
        assert_eq!(desc, error.description);
        assert_eq!(desc, error.description());
    }

    #[test]
    fn new() {
        let desc = "hello world";
        assert_error_eq(desc, &ParseError::new(desc));
    }

    #[test]
    fn err() {
        let desc = "hello world";
        let result = ParseError::err::<String>(desc);
        assert!(result.is_err());
        assert_error_eq(desc, &result.unwrap_err());
    }

    #[test]
    fn display() {
        assert_eq!("HTTP parsing error: hello world",
            format!("{}", ParseError::new("hello world")));
    }

    #[test]
    fn from_std_io_error() {
        let payload = "foo";
        let io_err = IoError::new(ErrorKind::ConnectionReset, payload);
        let pa_err = ParseError::from(io_err);
        assert_error_eq("Failed to read from connection", &pa_err);
    }
}