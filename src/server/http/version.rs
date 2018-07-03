use std::fmt;
use super::Error;

#[derive(Debug, PartialEq)]
pub struct Version {
    major: u8,
    minor: u8,
}

impl Version {
    pub fn new(major: u8, minor: u8) -> Version {
        Version{ major, minor }
    }

    pub fn from(version_str: &str) -> Result<Version, Error> {
        // version = HTTP-M.m
        // Split into bytes.
        let version: Vec<u8> = version_str.bytes().collect();
        if version.len() != 8 {
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