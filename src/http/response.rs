use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status_code: u16, status_text: &str) -> Self {
        Self {
            status_code,
            status_text: status_text.to_string(),
            headers: HashMap::new(),
            body: Vec::new(),
        }
    }
    
    pub fn set_header(&mut self, key: &str, value: &str) {
        self.headers.insert(key.to_string(), value.to_string());
    }
    
    pub fn set_body(&mut self, text: &str) {
        self.body = text.as_bytes().to_vec();
    }
    
    pub fn set_body_bytes(&mut self, bytes: Vec<u8>) {
        self.body = bytes;
    }
    
    /// Convert to wire format
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        
        // Status line
        output.extend_from_slice(
            format!("HTTP/1.1 {} {}\r\n", self.status_code, self.status_text).as_bytes()
        );
        
        // Headers
        for (key, value) in &self.headers {
            output.extend_from_slice(format!("{}: {}\r\n", key, value).as_bytes());
        }
        
        // Content-Length
        if !self.headers.contains_key("Content-Length") {
            output.extend_from_slice(
                format!("Content-Length: {}\r\n", self.body.len()).as_bytes()
            );
        }
        
        // End of headers
        output.extend_from_slice(b"\r\n");
        
        // Body
        output.extend_from_slice(&self.body);
        
        output
    }
    
    // Quick constructors
    
    pub fn ok() -> Self {
        Self::new(200, "OK")
    }
    
    pub fn not_found() -> Self {
        Self::new(404, "Not Found")
    }
    
    pub fn bad_request() -> Self {
        Self::new(400, "Bad Request")
    }
    
    pub fn method_not_allowed() -> Self {
        Self::new(405, "Method Not Allowed")
    }
    
    pub fn forbidden() -> Self {
        Self::new(403, "Forbidden")
    }
    
    pub fn internal_error() -> Self {
        Self::new(500, "Internal Server Error")
    }
    
    pub fn payload_too_large() -> Self {
        Self::new(413, "Payload Too Large")
    }
    
    pub fn ok_with_message(msg: &str) -> Self {
        let mut response = Self::ok();
        response.set_body(msg);
        response
    }
}
