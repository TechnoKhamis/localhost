use std::fs;
use std::path::Path;

/// Serve a file from disk
pub fn serve_file(file_path: &str) -> Result<Vec<u8>, String> {
    let path = Path::new(file_path);
    
    if !path.exists() {
        return Err("File not found".to_string());
    }
    
    match fs::read(path) {
        Ok(data) => Ok(data),
        Err(e) => Err(format!("Could not read file: {}", e)),
    }
}
