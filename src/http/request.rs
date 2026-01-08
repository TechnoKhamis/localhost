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

struct RequestLine {
    method: String,
    path: String,
    query: String,
    version: String,
}

impl HttpRequest {
    /// Parse HTTP request from buffer
    /// Returns None if not enough data yet (keep reading!)
    pub fn parse(buffer: &[u8]) -> Option<Self> {
        // Find headers boundary
        let headers_end = find_double_crlf(buffer)?;
        let header_section = &buffer[..headers_end];
        
        // Parse header section
        let header_text = std::str::from_utf8(header_section).ok()?;
        let mut lines = header_text.lines();
        
        let first_line = lines.next()?;
        let request_line_info = parse_request_line(first_line)?;
        let headers = parse_headers(lines);
        
        let body_start = headers_end + 4;
        
        // Check if we need to wait for body
        if let Some(len_str) = headers.get("Content-Length") {
            let expected_size: usize = len_str.parse().ok()?;
            let available = buffer.len().saturating_sub(body_start);
            
            println!("Content-Length: {}, Available: {}", expected_size, available);
            
            // Not enough data yet
            if available < expected_size {
                println!("Waiting for more body data...");
                return None;
            }
            
            // Extract body
            let body = buffer[body_start..body_start + expected_size].to_vec();
            println!("Full body received! {} bytes", body.len());
            
            
            return Some(HttpRequest {
                method: request_line_info.method,
                path: request_line_info.path,
                query: request_line_info.query,
                version: request_line_info.version,
                headers,
                body,
            });
        }
        
        // Check for chunked
        if let Some(te) = headers.get("Transfer-Encoding") {
            if te.to_lowercase().contains("chunked") {
                println!("Chunked encoding - parsing...");
                // Try to parse chunked data from buffer
                let body = parse_chunked_from_buffer(&buffer[body_start..])?;
                
                return Some(HttpRequest {
                    method: request_line_info.method,
                    path: request_line_info.path,
                    query: request_line_info.query,
                    version: request_line_info.version,
                    headers,
                    body,
                });
            }
        }
        
        // No body
        Some(HttpRequest {
            method: request_line_info.method,
            path: request_line_info.path,
            query: request_line_info.query,
            version: request_line_info.version,
            headers,
            body: Vec::new(),
        })
    }
}

fn find_double_crlf(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|w| w == b"\r\n\r\n")
}

fn parse_request_line(line: &str) -> Option<RequestLine> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?;
    let version = parts.next()?.to_string();
    
    let (path, query) = if let Some((p, q)) = target.split_once('?') {
        (p.to_string(), q.to_string())
    } else {
        (target.to_string(), String::new())
    };
    
    Some(RequestLine { method, path, query, version })
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

fn parse_chunked_from_buffer(data: &[u8]) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    let mut pos = 0;
    
    loop {
        // Find chunk size line end
        let line_end = data[pos..].windows(2).position(|w| w == b"\r\n")?;
        
        // Parse hex size
        let size_str = std::str::from_utf8(&data[pos..pos + line_end]).ok()?;
        let size = usize::from_str_radix(size_str.trim(), 16).ok()?;
        
        pos += line_end + 2; // Skip size + \r\n
        
        if size == 0 {
            break; // Last chunk
        }
        
        // Check data available
        if pos + size + 2 > data.len() {
            return None; // Need more data
        }
        
        // Extract chunk
        body.extend_from_slice(&data[pos..pos + size]);
        pos += size + 2; // Skip data + \r\n
    }
    
    Some(body)
}