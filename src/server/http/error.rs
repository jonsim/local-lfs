use std::fmt;
use std::error::Error as StdError;
use std::io::Error as IoError;
use std::num::ParseIntError;

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
