use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub query: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Parse HTTP request from buffer
    /// Returns None if request is incomplete
    pub fn parse(buffer: &[u8]) -> Option<Self> {
        // Find end of headers
        let headers_end = find_header_end(buffer)?;
        let header_section = &buffer[..headers_end];
        
        // Parse header text
        let header_text = std::str::from_utf8(header_section).ok()?;
        let mut lines = header_text.lines();
        
        // Parse request line
        let first_line = lines.next()?;
        let (method, path, query, version) = parse_request_line(first_line)?;
        
        // Parse headers
        let headers = parse_headers(lines);
        
        let body_start = headers_end + 4;
        
        // Handle Content-Length body
        if let Some(len_str) = headers.get("Content-Length") {
            let expected: usize = len_str.parse().ok()?;
            let available = buffer.len().saturating_sub(body_start);
            
            if available < expected {
                return None; // Need more data
            }
            
            let body = buffer[body_start..body_start + expected].to_vec();
            
            return Some(HttpRequest {
                method,
                path,
                query,
                version,
                headers,
                body,
            });
        }
        
        // Handle chunked encoding
        if let Some(te) = headers.get("Transfer-Encoding") {
            if te.to_lowercase().contains("chunked") {
                let body = parse_chunked(&buffer[body_start..])?;
                return Some(HttpRequest {
                    method,
                    path,
                    query,
                    version,
                    headers,
                    body,
                });
            }
        }
        
        // No body
        Some(HttpRequest {
            method,
            path,
            query,
            version,
            headers,
            body: Vec::new(),
        })
    }
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|w| w == b"\r\n\r\n")
}

fn parse_request_line(line: &str) -> Option<(String, String, String, String)> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?;
    let version = parts.next()?.to_string();
    
    let (path, query) = if let Some((p, q)) = target.split_once('?') {
        (p.to_string(), q.to_string())
    } else {
        (target.to_string(), String::new())
    };
    
    Some((method, path, query, version))
}

fn parse_headers<'a>(lines: impl Iterator<Item = &'a str>) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    headers
}

fn parse_chunked(data: &[u8]) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    let mut pos = 0;
    
    loop {
        if pos >= data.len() {
            return None; // Incomplete
        }
        
        // Find chunk size line end
        let remaining = &data[pos..];
        let line_end = remaining.windows(2).position(|w| w == b"\r\n")?;
        
        // Parse hex size
        let size_str = std::str::from_utf8(&remaining[..line_end]).ok()?;
        let size = usize::from_str_radix(size_str.trim(), 16).ok()?;
        
        pos += line_end + 2;
        
        if size == 0 {
            break; // End of chunks
        }
        
        if pos + size + 2 > data.len() {
            return None; // Need more data
        }
        
        body.extend_from_slice(&data[pos..pos + size]);
        pos += size + 2; // Skip chunk data + CRLF
    }
    
    Some(body)
}
