use std::path::{Path, PathBuf, Component};
use std::fs;
use crate::http::{HttpRequest, HttpResponse};

pub fn delete_file(request: &HttpRequest) -> HttpResponse {
    let file_path = extract_file_path(&request.path);
    let full_path = build_safe_path("uploads", &file_path);
    
    if !is_safe_path(&full_path) {
        println!("⚠️  Blocked unsafe path: {:?}", full_path);
        return HttpResponse::forbidden();
    }
    
    println!("🗑️  Attempting to delete: {:?}", full_path);
    
    match fs::remove_file(&full_path) {
        Ok(_) => {
            println!("✅ Deleted: {:?}", full_path);
            HttpResponse::ok_with_message("File deleted successfully")
        }
        Err(e) => {
            println!("❌ Delete failed: {}", e);
            HttpResponse::not_found()
        }
    }
}

fn extract_file_path(uri: &str) -> String {
    let mut path = uri.trim_start_matches('/');
    
    if path.starts_with("upload") {
        path = &path[6..];
        path = path.trim_start_matches('/');
    }
    
    path.to_string()
}

fn build_safe_path(base_dir: &str, relative: &str) -> PathBuf {
    let mut result = PathBuf::from(base_dir);  // Just "uploads", no ../
    
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(segment) => result.push(segment),
            Component::CurDir => {},
            _ => break,
        }
    }
    
    result
}

fn is_safe_path(path: &Path) -> bool {
    path.starts_with("uploads") && 
    !path.to_string_lossy().contains("..")
}