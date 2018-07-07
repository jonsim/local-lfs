use std::fmt;
use std::io::BufRead;
use super::Error;
use super::Field;
use super::Method;
use super::RequestStatus;
use super::Version;

#[derive(Debug, PartialEq)]
pub struct Request {
    line: RequestStatus,
    fields: Vec<Field>,
}

impl Request {
    pub fn new(line: RequestStatus, fields: Vec<Field>) -> Request {
        Request{ line, fields }
    }

    pub fn parse<B: BufRead>(reader: &mut B) -> Result<Request, Error>
    {
        let mut lines = reader.lines();
        let first_line = lines.next()
            .ok_or(Error::new("Unexpected end of stream"))??;
        let line = RequestStatus::from(first_line)?;
        let mut fields = Vec::new();
        loop {
            let iline = lines.next()
                .ok_or(Error::new("Unexpected end of stream"))??;
            if iline.is_empty() {
                break;  // header finished.
            }
            let field = Field::from(iline)?;
            fields.push(field);
        }

        Ok(Request{ line, fields })
    }

    pub fn version(&self) -> &Version {
        &self.line.version
    }

    pub fn method(&self) -> &Method {
        &self.line.method
    }

    pub fn target(&self) -> &str {
        &self.line.target
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut out = String::new();
        out.push_str(&format!("{}\r\n", self.line));
        for field in &self.fields {
            out.push_str(&format!("{}\r\n", field));
        }
        out.push_str("\r\n");
        f.pad(&out)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use super::super::tests::*;

    const STAT_METHOD: Method = Method::PATCH;
    const STAT_TARGET: &'static str = "target/";
    const FIELD_N1: &'static str = "foo";
    const FIELD_V1: &'static str = "bar";
    const FIELD_N2: &'static str = "hum";
    const FIELD_V2: &'static str = "bug";

    const REQUEST_ENCODING_0F: &'static str = "\
        PATCH target/ HTTP/1.1\r\n\
        \r\n";
    const REQUEST_ENCODING_1F: &'static str = "\
        PATCH target/ HTTP/1.1\r\n\
        foo: bar\r\n\
        \r\n";
    const REQUEST_ENCODING_2F: &'static str = "\
        PATCH target/ HTTP/1.1\r\n\
        foo:  bar\r\n\
        hum:bug\r\n\
        \r\n";
    const REQUEST_ENCODING_INVALID_STATUS: &'static str = "\
        PATCH_target/ HTTP/1.1\r\n\
        foo:  bar\r\n\
        hum:bug\r\n\
        \r\n";
    const REQUEST_ENCODING_INVALID_FIELDS: &'static str = "\
        PATCH target/ HTTP/1.1\r\n\
        hum :bug\r\n\
        \r\n";

    fn assert_field_eq(name: &str, value: &str, actual: &Field) {
        assert_eq!(String::from(name),  actual.name);
        assert_eq!(String::from(value), actual.value);
    }

    fn assert_request(request: &Request, field_num: usize) {
        let stat_version = Version::new(1, 1).unwrap();
        assert_eq!(stat_version, request.line.version);
        assert_eq!(STAT_METHOD,  request.line.method);
        assert_eq!(STAT_TARGET,  request.line.target);

        assert_eq!(field_num, request.fields.len());
        if field_num >= 1 {
            assert_field_eq(FIELD_N1, FIELD_V1, &request.fields[0]);
        }
        if field_num >= 2 {
            assert_field_eq(FIELD_N2, FIELD_V2, &request.fields[1]);
        }

        assert_eq!(&stat_version, request.version());
        assert_eq!(&STAT_METHOD,  request.method());
        assert_eq!(STAT_TARGET,   request.target());
    }

    #[test]
    fn new() {
        let status = RequestStatus::new(STAT_METHOD, String::from(STAT_TARGET));
        let mut fields: Vec<Field> = Vec::new();
        fields.push(Field::new(String::from(FIELD_N1), String::from(FIELD_V1)));
        fields.push(Field::new(String::from(FIELD_N2), String::from(FIELD_V2)));
        let request = Request::new(status, fields);
        assert_request(&request, 2);
    }

    #[test]
    fn parse_no_bytes() {
        let mut reader = StringReader::new("");
        let request = Request::parse(&mut reader);
        assert_parse_error("HTTP parsing error: Unexpected end of stream",
                           request);
    }

    #[test]
    fn parse_exact_bytes() {
        let mut reader = StringReader::new(REQUEST_ENCODING_0F);
        let request = Request::parse(&mut reader).unwrap();
        assert_request(&request, 0);

        let mut reader = StringReader::new(REQUEST_ENCODING_1F);
        let request = Request::parse(&mut reader).unwrap();
        assert_request(&request, 1);

        let mut reader = StringReader::new(REQUEST_ENCODING_2F);
        let request = Request::parse(&mut reader).unwrap();
        assert_request(&request, 2);
    }

    #[test]
    fn parse_invalid_status() {
        let mut reader = StringReader::new(REQUEST_ENCODING_INVALID_STATUS);
        let request = Request::parse(&mut reader);
        assert_parse_error("HTTP parsing error: Invalid request", request);
    }

    #[test]
    fn parse_invalid_fields() {
        let mut reader = StringReader::new(REQUEST_ENCODING_INVALID_FIELDS);
        let request = Request::parse(&mut reader);
        assert_parse_error("HTTP parsing error: Invalid field", request);
    }

    #[test]
    fn parse_truncated_status() {
        let mut reader = StringReader::new(&REQUEST_ENCODING_1F[..10]);
        let request = Request::parse(&mut reader);
        assert_parse_error("HTTP parsing error: Invalid request", request);
    }

    #[test]
    fn parse_truncated_field() {
        let mut reader = StringReader::new(&REQUEST_ENCODING_1F[..30]);
        let request = Request::parse(&mut reader);
        assert_parse_error("HTTP parsing error: Unexpected end of stream", request);
    }

    #[test]
    fn parse_too_many_bytes() {
        let mut long_request = String::from(REQUEST_ENCODING_1F);
        long_request.push_str(REQUEST_ENCODING_0F);
        let mut reader = StringReader::new(&long_request);
        let request = Request::parse(&mut reader).unwrap();
        assert_request(&request, 1);
    }

    #[test]
    fn display() {
        let mut reader = StringReader::new(&REQUEST_ENCODING_0F);
        let request = Request::parse(&mut reader).unwrap();
        assert_eq!(REQUEST_ENCODING_0F, format!("{}", request));

        let mut reader = StringReader::new(REQUEST_ENCODING_1F);
        let request = Request::parse(&mut reader).unwrap();
        assert_eq!(REQUEST_ENCODING_1F, format!("{}", request));

        let mut reader = StringReader::new(REQUEST_ENCODING_2F);
        let request = Request::parse(&mut reader).unwrap();
        assert_eq!("PATCH target/ HTTP/1.1\r\n\
                    foo: bar\r\n\
                    hum: bug\r\n\
                    \r\n", format!("{}", request));
    }
}
