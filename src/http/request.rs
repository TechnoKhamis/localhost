use std::collections::HashMap;

/// Parsed HTTP request
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub query: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// Request line components (e.g., "GET /path?q=1 HTTP/1.1")
struct RequestLine {
    method: String,
    path: String,
    query: String,
    version: String,
}

impl HttpRequest {
    /// Parse HTTP request from raw bytes
    pub fn parse(buffer: &[u8]) -> Option<Self> {
        // Find headers boundary
        let headers_end = find_double_crlf(buffer)?;
        let header_section = &buffer[..headers_end];
        let body = buffer[headers_end + 4..].to_vec();
        
        // Parse header section
        let header_text = std::str::from_utf8(header_section).ok()?;
        let mut lines = header_text.lines();
        
        let first_line = lines.next()?;
        let request_line_info = parse_request_line(first_line)?;
        let headers = parse_headers(lines);
        
        Some(HttpRequest {
            method: request_line_info.method,
            path: request_line_info.path,
            query: request_line_info.query,
            version: request_line_info.version,
            headers,
            body,
        })
    }
}

/// Find "\r\n\r\n" position
fn find_double_crlf(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Parse "GET /path?q=1 HTTP/1.1"
fn parse_request_line(line: &str) -> Option<RequestLine> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?;
    let version = parts.next()?.to_string();
    
    // Split path and query string
    let (path, query) = if let Some((p, q)) = target.split_once('?') {
        (p.to_string(), q.to_string())
    } else {
        (target.to_string(), String::new())
    };
    
    Some(RequestLine { method, path, query, version })
}

/// Parse "Key: Value" lines
fn parse_headers<'a>(lines: impl Iterator<Item = &'a str>) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    headers
}