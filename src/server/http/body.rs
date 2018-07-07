use std::fmt;
use std::io::Read;
use super::Error;

#[derive(Debug, PartialEq)]
pub struct Body {
    content: Vec<u8>,
}

impl Body {
    pub fn parse<R: Read>(reader: &mut R, length: usize) -> Result<Body, Error> {
        let mut content: Vec<u8> = vec![0; length];
        match reader.read_exact(content.as_mut_slice()) {
            Ok(_) => Ok(Body{ content }),
            Err(_) => Err(Error::new("Failed to read requested bytes")),
        }
        
    }

    pub fn from(string: String) -> Body {
        Body{ content: string.into_bytes() }
    }

    pub fn content_length(&self) -> usize {
        self.content.len()
    }
}

impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{}", String::from_utf8_lossy(&self.content)))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::*;

    fn assert_body_empty(body: &Body) {
        let empty_vec: Vec<u8> = Vec::new();
        assert_eq!(empty_vec, body.content);
        assert_eq!(0, body.content_length());
        assert_eq!("", format!("{}", body));
    }

    fn assert_body_eq(expected: String, actual: &Body, ) {
        assert_eq!(expected, format!("{}", actual));
        let expected = expected.into_bytes();
        assert_eq!(expected, actual.content);
        assert_eq!(expected.len(), actual.content_length());
    }

    #[test]
    fn from_empty_string() {
        let body = Body::from(String::new());
        assert_body_empty(&body);
    }

    #[test]
    fn from_nonempty_string() {
        let expected = String::from("hello world");
        let body = Body::from(expected.clone());
        assert_body_eq(expected, &body);
    }

    #[test]
    fn parse_no_bytes() {
        let mut reader = StringReader::new("");
        let body = Body::parse(&mut reader, 0).unwrap();
        assert_body_empty(&body);
    }

    #[test]
    fn parse_exact_bytes() {
        let expected = "hello world";
        let mut reader = StringReader::new(expected);
        let body = Body::parse(&mut reader, 11).unwrap();
        assert_body_eq(String::from(expected), &body);
    }

    #[test]
    fn parse_too_many_bytes() {
        let expected = "hello world";
        let mut reader = StringReader::new(expected);
        let body = Body::parse(&mut reader, 4).unwrap();
        assert_body_eq(String::from(&expected[..4]), &body);
    }

    #[test]
    fn parse_too_few_bytes() {
        let expected = "hello world";
        let mut reader = StringReader::new(expected);
        let result = Body::parse(&mut reader, 20);
        assert_parse_error("HTTP parsing error: Failed to read requested bytes",
                result);
    }
}