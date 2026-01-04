use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Create new response
    pub fn new(status_code: u16, status_text: &str) -> Self {
        Self {
            status_code,
            status_text: status_text.to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }
    
    /// Set header
    pub fn set_header(&mut self, key: &str, value: &str) {
        self.headers.insert(key.to_string(), value.to_string());
    }
    
    /// Set body from text
    pub fn set_body(&mut self, text: &str) {
        self.body = text.as_bytes().to_vec();
    }
    
    pub fn set_body_bytes(&mut self, bytes: Vec<u8>) {
        self.body = bytes;
    }

    
    /// Convert to HTTP bytes for sending over network
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        
        // Status line: "HTTP/1.1 200 OK\r\n"
        let status_line = format!("HTTP/1.1 {} {}\r\n", self.status_code, self.status_text);
        output.extend_from_slice(status_line.as_bytes());
        
        // Headers
        for (key, value) in &self.headers {
            let header = format!("{}: {}\r\n", key, value);
            output.extend_from_slice(header.as_bytes());
        }
        
        // Content-Length header (auto-add if not present)
        if !self.headers.contains_key("Content-Length") {
            let content_length = format!("Content-Length: {}\r\n", self.body.len());
            output.extend_from_slice(content_length.as_bytes());
        }
        
        // Empty line
        output.extend_from_slice(b"\r\n");
        
        // Body
        output.extend_from_slice(&self.body);
        
        output
    }
    
    // Quick constructors for common responses
    
    pub fn ok() -> Self {
        Self::new(200, "OK")
    }
    
    pub fn not_found() -> Self {
        Self::new(404, "Not Found")
    }
    
    pub fn internal_error() -> Self {
        Self::new(500, "Internal Server Error")
    }
    
    pub fn bad_request() -> Self {
        Self::new(400, "Bad Request")
    }

    pub fn method_not_allowed() -> Self {
        Self::new(405, "Method Not Allowed") 
    }
}