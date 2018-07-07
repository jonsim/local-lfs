use std::fmt;
use std::io::BufRead;
use super::Error;
use super::Field;
use super::ResponseStatus;
use super::StatusCode;
use super::Version;

#[derive(Debug, PartialEq)]
pub struct Response {
    line: ResponseStatus,
    fields: Vec<Field>,
}

impl Response {
    pub fn new(line: ResponseStatus, fields: Vec<Field>) -> Response {
        Response{ line, fields }
    }

    pub fn parse<B: BufRead>(reader: &mut B) -> Result<Response, Error>
    {
        let mut lines = reader.lines();
        let first_line = lines.next()
            .ok_or(Error::new("Unexpected end of stream"))??;
        let line = ResponseStatus::from(first_line)?;
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

        Ok(Response{ line, fields })
    }

    pub fn version(&self) -> &Version {
        &self.line.version
    }

    pub fn status(&self) -> &StatusCode {
        &self.line.status
    }
}

impl fmt::Display for Response {
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

    const STAT_CODE: StatusCode = StatusCode::PayloadTooLarge;
    const FIELD_N1: &'static str = "foo";
    const FIELD_V1: &'static str = "bar";
    const FIELD_N2: &'static str = "hum";
    const FIELD_V2: &'static str = "bug";

    const RESPONSE_ENCODING_0F: &'static str = "\
        HTTP/1.1 413 Payload Too Large\r\n\
        \r\n";
    const RESPONSE_ENCODING_1F: &'static str = "\
        HTTP/1.1 413 Payload too girthy\r\n\
        foo: bar\r\n\
        \r\n";
    const RESPONSE_ENCODING_2F: &'static str = "\
        HTTP/1.1 413 omg\r\n\
        foo:  bar\r\n\
        hum:bug\r\n\
        \r\n";
    const RESPONSE_ENCODING_INVALID_STATUS: &'static str = "\
        HTTP/1.1 Payload too large\r\n\
        foo:  bar\r\n\
        hum:bug\r\n\
        \r\n";
    const RESPONSE_ENCODING_INVALID_FIELDS: &'static str = "\
        HTTP/1.1 413 Payload too large\r\n\
        hum :bug\r\n\
        \r\n";

    fn assert_field_eq(name: &str, value: &str, actual: &Field) {
        assert_eq!(String::from(name),  actual.name);
        assert_eq!(String::from(value), actual.value);
    }

    fn assert_response(response: &Response, field_num: usize) {
        let stat_version = Version::new(1, 1).unwrap();
        assert_eq!(stat_version, response.line.version);
        assert_eq!(STAT_CODE,    response.line.status);

        assert_eq!(field_num, response.fields.len());
        if field_num >= 1 {
            assert_field_eq(FIELD_N1, FIELD_V1, &response.fields[0]);
        }
        if field_num >= 2 {
            assert_field_eq(FIELD_N2, FIELD_V2, &response.fields[1]);
        }

        assert_eq!(&stat_version, response.version());
        assert_eq!(&STAT_CODE,    response.status());
    }

    #[test]
    fn new() {
        let status = ResponseStatus::new(STAT_CODE);
        let mut fields: Vec<Field> = Vec::new();
        fields.push(Field::new(String::from(FIELD_N1), String::from(FIELD_V1)));
        fields.push(Field::new(String::from(FIELD_N2), String::from(FIELD_V2)));
        let response = Response::new(status, fields);
        assert_response(&response, 2);
    }

    #[test]
    fn parse_no_bytes() {
        let mut reader = StringReader::new("");
        let response = Response::parse(&mut reader);
        assert_parse_error("HTTP parsing error: Unexpected end of stream",
                           response);
    }

    #[test]
    fn parse_exact_bytes() {
        let mut reader = StringReader::new(RESPONSE_ENCODING_0F);
        let response = Response::parse(&mut reader).unwrap();
        assert_response(&response, 0);

        let mut reader = StringReader::new(RESPONSE_ENCODING_1F);
        let response = Response::parse(&mut reader).unwrap();
        assert_response(&response, 1);

        let mut reader = StringReader::new(RESPONSE_ENCODING_2F);
        let response = Response::parse(&mut reader).unwrap();
        assert_response(&response, 2);
    }

    #[test]
    fn parse_invalid_status() {
        let mut reader = StringReader::new(RESPONSE_ENCODING_INVALID_STATUS);
        let response = Response::parse(&mut reader);
        assert_parse_error("HTTP parsing error: Invalid status", response);
    }

    #[test]
    fn parse_invalid_fields() {
        let mut reader = StringReader::new(RESPONSE_ENCODING_INVALID_FIELDS);
        let response = Response::parse(&mut reader);
        assert_parse_error("HTTP parsing error: Invalid field", response);
    }

    #[test]
    fn parse_truncated_status() {
        let mut reader = StringReader::new(&RESPONSE_ENCODING_1F[..10]);
        let response = Response::parse(&mut reader);
        assert_parse_error("HTTP parsing error: Invalid response", response);
    }

    #[test]
    fn parse_truncated_field() {
        let mut reader = StringReader::new(&RESPONSE_ENCODING_1F[..30]);
        let response = Response::parse(&mut reader);
        assert_parse_error("HTTP parsing error: Unexpected end of stream", response);
    }

    #[test]
    fn parse_too_many_bytes() {
        let mut long_response = String::from(RESPONSE_ENCODING_1F);
        long_response.push_str(RESPONSE_ENCODING_0F);
        let mut reader = StringReader::new(&long_response);
        let response = Response::parse(&mut reader).unwrap();
        assert_response(&response, 1);
    }

    #[test]
    fn display() {
        let mut reader = StringReader::new(&RESPONSE_ENCODING_0F);
        let response = Response::parse(&mut reader).unwrap();
        assert_eq!(RESPONSE_ENCODING_0F, format!("{}", response));

        let mut reader = StringReader::new(RESPONSE_ENCODING_1F);
        let response = Response::parse(&mut reader).unwrap();
        assert_eq!("HTTP/1.1 413 Payload Too Large\r\n\
                    foo: bar\r\n\
                    \r\n", format!("{}", response));

        let mut reader = StringReader::new(RESPONSE_ENCODING_2F);
        let response = Response::parse(&mut reader).unwrap();
        assert_eq!("HTTP/1.1 413 Payload Too Large\r\n\
                    foo: bar\r\n\
                    hum: bug\r\n\
                    \r\n", format!("{}", response));
    }


}