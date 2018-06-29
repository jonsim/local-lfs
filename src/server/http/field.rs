use std::fmt;
use super::Error;

#[derive(Debug)]
pub struct Field {
    name:  String,
    value: String,
}

impl Field {
    pub fn new(name: String, value: String) -> Field {
        Field{ name, value }
    }

    pub fn ContentLength(length: usize) -> Field {
        let name = String::from("Content-Length");
        let value = format!("{}", length);
        Field{ name, value }
    }

    pub fn from(line: String) -> Result<Field, Error> {
        // header-field   = field-name ":" OWS field-value OWS
        // field-value    = *( field-content / obs-fold )
        // field-content  = field-vchar [ 1*( SP / HTAB ) field-vchar ]
        // field-vchar    = VCHAR / obs-text
        // obs-fold       = CRLF 1*( SP / HTAB )
        //                ; obsolete line folding (see Section 3.2.4)
        // Split by the first colon separator.
        let sep = line.find(':');
        if sep.is_none() {
            return Error::err("Bad HTTP header");
        }
        // Parse name. Names must not contain whitespace.
        let name = String::from(&line[..sep.unwrap()]);
        if name.find(char::is_whitespace).is_some() {
            return Error::err("Bad HTTP header");
        }
        // Parse value. Values must have leading/trailing whitespace removed.
        // Line folding unsupported.
        let value = String::from(line[sep.unwrap()+1..].trim());
        if value.find('\n').is_some() {
            return Error::err("Bad HTTP header");
        }

        Ok(Field{ name, value })
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("{}: {}", self.name, self.value))
    }
}
