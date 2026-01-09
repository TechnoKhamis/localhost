use crate::http::{HttpRequest, HttpResponse};
use std::fs::{self, File};
use std::io::Write;

pub fn upload_file(request: &HttpRequest, max_size: usize, error_path: &str) -> HttpResponse {
    if request.body.len() > max_size {
        return error_response(413, error_path, "Payload Too Large");
    }
    
    let ct = request.headers
        .get("Content-Type")
        .map(|v| v.as_str())
        .unwrap_or("");
    
    if ct.to_ascii_lowercase().starts_with("multipart/form-data") {
        return handle_multipart(ct, &request.body, max_size, error_path);
    }
    
    let filename = request.headers
        .get("X-Filename")
        .map(|v| v.to_string())
        .unwrap_or_else(|| format!("upload-{}.bin", timestamp_ms()));
    
    save_file("uploads", &filename, &request.body, error_path)
}

fn handle_multipart(ct: &str, body: &[u8], max_size: usize, error_path: &str) -> HttpResponse {
    let boundary = match extract_boundary(ct) {
        Some(b) => b,
        None => return error_response(400, error_path, "Bad Request"),
    };
    
    let delim = format!("--{}", boundary).into_bytes();
    
    // Find header end
    let mut pos = 0;
    for i in 0..body.len().saturating_sub(3) {
        if &body[i..i+4] == b"\r\n\r\n" {
            pos = i + 4;
            break;
        }
    }
    
    // Extract filename
    let headers = String::from_utf8_lossy(&body[0..pos]);
    let filename = headers
        .split("filename=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("upload.bin")
        .to_string();
    
    // Find next boundary
    let data_end = body[pos..]
        .windows(delim.len())
        .position(|w| w == &delim[..])
        .map(|i| pos + i - 2)
        .unwrap_or(body.len());
    
    let file_data = &body[pos..data_end];
    
    if file_data.len() > max_size {
        return error_response(413, error_path, "Payload Too Large");
    }
    
    save_file("uploads", &filename, file_data, error_path)
}

fn extract_boundary(ct: &str) -> Option<String> {
    ct.split(';')
        .find(|p| p.trim().starts_with("boundary="))
        .map(|p| p.split('=').nth(1).unwrap_or("").trim().trim_matches('"').to_string())
}

fn save_file(dir: &str, filename: &str, data: &[u8], error_path: &str) -> HttpResponse {
    let _ = fs::create_dir_all(dir);
    
    let safe = sanitize_filename(filename);
    let path = format!("{}/{}", dir, safe);
    
    match File::create(&path).and_then(|mut f| f.write_all(data)) {
        Ok(_) => HttpResponse::ok_with_message(&format!("File '{}' uploaded successfully", safe)),
        Err(_) => error_response(500, error_path, "Internal Server Error"),
    }
}

fn error_response(code: u16, error_path: &str, message: &str) -> HttpResponse {
    let error_file = format!("{}/{}.html", error_path, code);
    
    match crate::handlers::serve_file(&error_file) {
        Ok(content) => {
            let mut response = HttpResponse::new(code, message);
            response.set_header("Content-Type", "text/html");
            response.set_body_bytes(content);
            response
        }
        Err(_) => {
            // Fallback if error page not found
            let mut response = HttpResponse::new(code, message);
            response.set_header("Content-Type", "text/html");
            response.set_body(&format!("<!DOCTYPE html><html><body><h1>{} - {}</h1></body></html>", code, message));
            response
        }
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect()
}

fn timestamp_ms() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
