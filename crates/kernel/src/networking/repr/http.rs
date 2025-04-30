use crate::networking::{Error, Result};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Patch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub method: Method,
    pub path: String,
    pub version: String,
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
}

impl Packet {
    pub fn new(method: Method, host: &str, path: &str) -> Self {
        let mut headers = Vec::new();
        headers.push(Header {
            name: "Host".to_string(),
            value: host.to_string(),
        });
        headers.push(Header {
            name: "User-Agent".to_string(),
            value: "curl/8.13.0".to_string(),
        });
        headers.push(Header {
            name: "Accept".to_string(),
            value: "*/*".to_string(),
        });
        Packet {
            method,
            path: path.to_string(),
            version: "HTTP/1.1".to_string(),
            headers,
            body: Vec::new(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Start-line
        let method_str = match self.method {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
            Method::Patch => "PATCH",
        };

        buffer.extend_from_slice(method_str.as_bytes());
        buffer.push(b' ');
        buffer.extend_from_slice(self.path.as_bytes());
        buffer.push(b' ');
        buffer.extend_from_slice(self.version.as_bytes());
        buffer.extend_from_slice(b"\r\n");

        // Headers
        for header in &self.headers {
            buffer.extend_from_slice(header.name.as_bytes());
            buffer.extend_from_slice(b": ");
            buffer.extend_from_slice(header.value.as_bytes());
            buffer.extend_from_slice(b"\r\n");
        }

        // End of headers
        buffer.extend_from_slice(b"\r\n");

        // Body
        buffer.extend_from_slice(&self.body);

        buffer
    }

    pub fn deserialize(buffer: &[u8]) -> Result<Self> {
        let mut headers = Vec::new();
        let mut pos = 0;

        // Parse request line
        let request_line_end = find_crlf(buffer, pos).ok_or(Error::Malformed)?;
        let request_line = &buffer[pos..request_line_end];
        let parts = split_ascii_whitespace(request_line);

        if parts.len() != 3 {
            return Err(Error::Malformed);
        }

        let method = match parts[0] {
            b"GET" => Method::Get,
            b"POST" => Method::Post,
            b"PUT" => Method::Put,
            b"DELETE" => Method::Delete,
            b"HEAD" => Method::Head,
            b"OPTIONS" => Method::Options,
            b"PATCH" => Method::Patch,
            _ => return Err(Error::Malformed),
        };

        let path = String::from_utf8(parts[1].to_vec()).map_err(|_| Error::Malformed)?;
        let version = String::from_utf8(parts[2].to_vec()).map_err(|_| Error::Malformed)?;

        pos = request_line_end + 2;

        // Parse headers
        loop {
            if pos >= buffer.len() {
                return Err(Error::Malformed);
            }

            if buffer[pos..].starts_with(b"\r\n") {
                pos += 2;
                break;
            }

            let header_end = find_crlf(buffer, pos).ok_or(Error::Malformed)?;
            let header_line = &buffer[pos..header_end];

            if let Some(colon_pos) = header_line.iter().position(|&b| b == b':') {
                let name = String::from_utf8(header_line[..colon_pos].to_vec())
                    .map_err(|_| Error::Malformed)?;
                let value = String::from_utf8(header_line[colon_pos + 1..].to_vec())
                    .map_err(|_| Error::Malformed)?
                    .trim()
                    .to_string();
                headers.push(Header { name, value });
            } else {
                return Err(Error::Malformed);
            }

            pos = header_end + 2;
        }

        let body = buffer[pos..].to_vec();

        Ok(Packet {
            method,
            path,
            version,
            headers,
            body,
        })
    }

    pub fn get_header(&self, name: &str) -> Option<&str> {
        for header in &self.headers {
            if header.name.eq_ignore_ascii_case(name) {
                return Some(&header.value);
            }
        }
        None
    }

    pub fn content_length(&self) -> Option<usize> {
        if let Some(value) = self.get_header("Content-Length") {
            value.parse().ok()
        } else {
            None
        }
    }
}

// Helper: Find \r\n (CRLF) sequence starting from position
fn find_crlf(buffer: &[u8], start: usize) -> Option<usize> {
    buffer[start..]
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|p| start + p)
}

// Helper: Split a line into whitespace-separated parts
fn split_ascii_whitespace(line: &[u8]) -> Vec<&[u8]> {
    let mut parts = Vec::new();
    let mut start = None;
    for (i, &b) in line.iter().enumerate() {
        if b.is_ascii_whitespace() {
            if let Some(s) = start {
                parts.push(&line[s..i]);
                start = None;
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        parts.push(&line[s..]);
    }
    parts
}
