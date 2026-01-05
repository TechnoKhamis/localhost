use crate::http::{HttpRequest, HttpResponse};
use std::fs::{self, File};
use std::io::Write;

pub fn upload_file(request: &HttpRequest, max_size: usize) -> HttpResponse {
    if request.body.len() > max_size {
        return HttpResponse::payload_too_large();
    }
    
    let ct = request.headers
        .get("Content-Type")
        .map(|v| v.as_str())
        .unwrap_or("");
    
    if ct.to_ascii_lowercase().starts_with("multipart/form-data") {
        return handle_multipart(ct, &request.body, max_size);
    }
    
    let filename = request.headers
        .get("X-Filename")
        .map(|v| v.to_string())
        .unwrap_or_else(|| format!("upload-{}.bin", now_epoch_ms()));
    
    save_file("uploads", &filename, &request.body)
}

fn handle_multipart(ct: &str, body: &[u8], max_size: usize) -> HttpResponse {
    let boundary = match extract_boundary(ct) {
        Some(b) => b,
        None => return HttpResponse::bad_request(),
    };
    
    let delim = format!("--{}", boundary).into_bytes();
    
    // Find "\r\n\r\n" (headers end)
    let mut pos = 0;
    for i in 0..body.len().saturating_sub(3) {
        if &body[i..i+4] == b"\r\n\r\n" {
            pos = i + 4;
            break;
        }
    }
    
    // Extract filename from headers
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
        return HttpResponse::payload_too_large();
    }
    
    save_file("uploads", &filename, file_data)
}

fn extract_boundary(ct: &str) -> Option<String> {
    ct.split(';')
        .find(|p| p.trim().starts_with("boundary="))
        .map(|p| p.split('=').nth(1).unwrap_or("").trim().to_string())
}

fn save_file(dir: &str, filename: &str, data: &[u8]) -> HttpResponse {
    // Remove the ../
    fs::create_dir_all(dir).ok();  // Just "uploads"
    
    let safe = sanitize_filename(filename);
    let path = format!("{}/{}", dir, safe);  // Just "uploads/file.txt"
    
    match File::create(&path).and_then(|mut f| f.write_all(data)) {
        Ok(_) => {
            println!("✅ Saved: {} ({} bytes)", path, data.len());
            HttpResponse::ok()
        }
        Err(e) => {
            println!("❌ Error: {}", e);
            HttpResponse::internal_error()
        }
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect()
}

fn now_epoch_ms() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}