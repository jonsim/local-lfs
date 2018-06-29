use std::fmt;

#[derive(Debug)]
pub struct Body {
    content: String,
}

impl Body {
    pub fn from(content: String) -> Body {
        Body{ content }
    }
    pub fn content_length(&self) -> usize {
        self.content.len()
    }
}

impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{}", self.content))
    }
}
