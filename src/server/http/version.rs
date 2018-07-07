use std::fmt;
use super::Error;

#[derive(Debug, PartialEq)]
pub struct Version {
    major: u8,
    minor: u8,
}

impl Version {
    pub fn new(major: u8, minor: u8) -> Result<Version, Error> {
        if major > 9 || minor > 9 {
            Error::err("Invalid version")
        } else {
            Ok(Version{ major, minor })
        }
    }

    pub fn from(version_str: &str) -> Result<Version, Error> {
        // version = HTTP-M.m
        // Split into bytes.
        let version: Vec<u8> = version_str.bytes().collect();
        if version.len() != 8 ||
           version[0] != b'H' ||
           version[1] != b'T' ||
           version[2] != b'T' ||
           version[3] != b'P' ||
           version[4] != b'/' ||
           version[6] != b'.' {
            return Error::err("Invalid version");
        }
        // Parse major and minor versions.
        let major: u8 = version[5] - b'0';
        let minor: u8 = version[7] - b'0';
        if major > 9 || minor > 9 {
            return Error::err("Invalid version");
        }

        Ok(Version{ major, minor })
    }

    pub fn major(&self) -> u8 {
        self.major
    }

    pub fn minor(&self) -> u8 {
        self.minor
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(&format!("HTTP/{}.{}", self.major, self.minor))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::*;

    fn assert_version_eq(major: u8, minor: u8, actual: &Version) {
        assert_eq!(major, actual.major);
        assert_eq!(minor, actual.minor);
        assert_eq!(major, actual.major());
        assert_eq!(minor, actual.minor());
    }

    #[test]
    fn new_valid() {
        for ma in 0..9 {
            for mi in 0..9 {
                assert_version_eq(ma, mi, &Version::new(ma, mi).unwrap());
            }
        }
    }

    #[test]
    fn new_invalid() {
        for ma in 10..255 {
            for mi in 10..255 {
                assert_parse_error("HTTP parsing error: Invalid version",
                    Version::new(ma, mi));
            }
        }
    }

    #[test]
    fn from_valid_string() {
        for ma in 0..9 {
            for mi in 0..9 {
                let v = Version::from(&format!("HTTP/{}.{}", ma, mi)).unwrap();
                assert_version_eq(ma, mi, &v);
            }
        }
    }

    #[test]
    fn from_invalid_empty() {
        let v = Version::from("");
        assert_parse_error("HTTP parsing error: Invalid version", v);
    }

    #[test]
    fn from_invalid_too_short() {
        // Not enough numbers.
        let v = Version::from("HTTP/1.");
        assert_parse_error("HTTP parsing error: Invalid version", v);

        // Not enough pre-amble.
        let v = Version::from("HTP/1.1");
        assert_parse_error("HTTP parsing error: Invalid version", v);

        // Not enough separators.
        let v = Version::from("HTTP1.1");
        assert_parse_error("HTTP parsing error: Invalid version", v);
    }

    #[test]
    fn from_invalid_too_long() {
        // Too many numbers.
        let v = Version::from("HTTP/1.11");
        assert_parse_error("HTTP parsing error: Invalid version", v);

        // Too much pre-amble.
        let v = Version::from("HTTTP/1.1");
        assert_parse_error("HTTP parsing error: Invalid version", v);

        // Too many separators.
        let v = Version::from("HTTP//1.1");
        assert_parse_error("HTTP parsing error: Invalid version", v);
    }

    #[test]
    fn from_invalid_protocol() {
        let v = Version::from("HTIP/1.1");
        assert_parse_error("HTTP parsing error: Invalid version", v);
    }

    #[test]
    fn from_invalid_separator() {
        let v = Version::from("HTTP-1.1");
        assert_parse_error("HTTP parsing error: Invalid version", v);
    }

    #[test]
    fn from_invalid_point() {
        let v = Version::from("HTTP/1,1");
        assert_parse_error("HTTP parsing error: Invalid version", v);
    }

    #[test]
    fn from_invalid_non_numeric() {
        // Wrong major version.
        let v = Version::from("HTTP/l.1");
        assert_parse_error("HTTP parsing error: Invalid version", v);

        // Wrong minor version.
        let v = Version::from("HTTP/1.I");
        assert_parse_error("HTTP parsing error: Invalid version", v);
    }

    #[test]
    fn display() {
        let s = "HTTP/4.2";
        assert_eq!(s, format!("{}", Version::from(s).unwrap()));
    }
}