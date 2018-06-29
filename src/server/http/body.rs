use std::fmt;
use std::io::Read;
use super::Error;

#[derive(Debug)]
pub struct Body {
    content: Vec<u8>,
}

impl Body {
    pub fn parse<R: Read>(reader: &mut R, length: usize) -> Result<Body, Error> {
        let mut content = Vec::with_capacity(length);
        reader.read_exact(content.as_mut_slice())?;
        Ok(Body{ content })
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
