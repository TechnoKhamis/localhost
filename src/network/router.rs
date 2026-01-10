use crate::config::{RouteConfig, ServerConfig, VHost};
use crate::http::{HttpRequest, HttpResponse};
use std::path::{Path, PathBuf, Component};

/// Sanitize and validate a path to prevent directory traversal attacks
fn sanitize_path(base: &str, user_path: &str) -> Option<PathBuf> {
    let base_path = Path::new(base).canonicalize().unwrap_or_else(|_| PathBuf::from(base));
    
    // Decode URL encoding (handle %2F, %2E, etc.)
    let decoded = url_decode(user_path);
    
    // Build path safely, rejecting any traversal attempts
    let mut result = PathBuf::from(base);
    
    for component in Path::new(&decoded).components() {
        match component {
            Component::Normal(seg) => {
                // Check for null bytes
                let seg_str = seg.to_string_lossy();
                if seg_str.contains('\0') {
                    return None;
                }
                result.push(seg);
            }
            Component::CurDir => {} // "." is safe, just skip
            Component::ParentDir => {
                // ".." - REJECT! This is a traversal attempt
                return None;
            }
            Component::RootDir | Component::Prefix(_) => {
                // Absolute path attempt - REJECT!
                return None;
            }
        }
    }
    
    // Verify the final path is still under base directory
    if let Ok(canonical) = result.canonicalize() {
        if canonical.starts_with(&base_path) {
            return Some(result);
        }
    } else {
        // File doesn't exist yet, but path is constructed safely
        // Check that it would be under base if it existed
        if result.starts_with(base) && !result.to_string_lossy().contains("..") {
            return Some(result);
        }
    }
    
    None
}

/// URL decode a string (handles %XX encoding)
fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    
    while let Some(c) = chars.next() {
        if c == '%' {
            // Try to read two hex digits
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            // Invalid encoding, keep as-is
            result.push('%');
            result.push_str(&hex);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    
    result
}

/// Check if a path contains dangerous patterns
fn is_path_safe(path: &str) -> bool {
    let decoded = url_decode(path);
    
    // Check for obvious traversal patterns
    if decoded.contains("..") {
        return false;
    }
    
    // Check for null bytes
    if decoded.contains('\0') {
        return false;
    }
    
    // Check for double slashes that might confuse path parsing
    if decoded.contains("//") {
        return false;
    }
    
    true
}

pub fn find_route<'a>(
    request: &HttpRequest,
    routes: &'a [RouteConfig],
) -> Option<&'a RouteConfig> {
    routes
        .iter()
        .filter(|route| request.path.starts_with(&route.path))
        .max_by_key(|route| route.path.len())
}

pub fn route_request(
    request: &HttpRequest,
    config: &ServerConfig,
) -> HttpResponse {
    // Check HTTP version FIRST
    if request.version != "HTTP/1.1" {
        return error_response(400, &config.error_path, "Bad Request");
    }

    // SECURITY: Check for path traversal attempts EARLY
    if !is_path_safe(&request.path) {
        return error_response(403, &config.error_path, "Forbidden");
    }

    // Check body size limit EARLY (before any processing)
    if request.body.len() > config.client_body_size_limit {
        return error_response(413, &config.error_path, "Payload Too Large");
    }

    let (routes, error_path) = if !config.vhosts.is_empty() {
        if let Some(vhost) = find_vhost(request, &config.vhosts) {
            (&vhost.routes, &vhost.error_path)
        } else {
            (&config.routes, &config.error_path)
        }
    } else {
        (&config.routes, &config.error_path)
    };
    
    let matched = find_route(request, routes);
    
    match matched {
        Some(route) => {
            // Check method
            if !route.methods.iter().any(|m| m.eq_ignore_ascii_case(&request.method)) {
                return error_response(405, error_path, "Method Not Allowed");
            }
            
            // Handle redirect
            if let Some(target) = &route.redirect {
                let mut resp = HttpResponse::new(302, "Found");
                resp.set_header("Location", target);
                return resp;
            }
            
            // Handle upload
            if route.path.contains("/upload") {
                if request.method.eq_ignore_ascii_case("POST") {
                    // Double check size limit for uploads and return proper error page
                    if request.body.len() > config.client_body_size_limit {
                        return error_response(413, error_path, "Payload Too Large");
                    }
                    return crate::handlers::upload_file(request, config.client_body_size_limit, error_path);
                } else if request.method.eq_ignore_ascii_case("DELETE") {
                    return crate::handlers::delete_file(request);
                }
            }
            
            // Handle CGI
            if let Some(_cgi) = &route.cgi {
                // Extract script name from URL path
                // e.g., /cgi/test.py -> test.py
                let after_route = request.path
                    .strip_prefix(&route.path)
                    .unwrap_or(&request.path)
                    .trim_start_matches('/');
                
                // Handle empty script name
                if after_route.is_empty() {
                    return error_response(404, error_path, "Not Found");
                }
                
                // SECURITY: Validate CGI script path
                let script_path = match sanitize_path(&route.root, after_route) {
                    Some(safe_path) => safe_path.to_string_lossy().to_string(),
                    None => return error_response(403, error_path, "Forbidden"),
                };
                
                let path_info = request.path.clone();
                return crate::handlers::run_cgi(&script_path, &path_info, request);
            }
            
            // Build file path SAFELY
            let file_path = if route.path == "/" {
                let req_path = request.path.trim_start_matches('/');
                if req_path.is_empty() {
                    if let Some(df) = &route.default_file {
                        format!("{}/{}", route.root, df)
                    } else {
                        route.root.clone()
                    }
                } else {
                    // SECURITY: Sanitize the path
                    match sanitize_path(&route.root, req_path) {
                        Some(safe_path) => safe_path.to_string_lossy().to_string(),
                        None => return error_response(403, error_path, "Forbidden"),
                    }
                }
            } else {
                let after = request.path.strip_prefix(&route.path)
                    .unwrap_or("")
                    .trim_start_matches('/');
                
                if after.is_empty() {
                    route.root.clone()
                } else {
                    // SECURITY: Sanitize the path
                    match sanitize_path(&route.root, after) {
                        Some(safe_path) => safe_path.to_string_lossy().to_string(),
                        None => return error_response(403, error_path, "Forbidden"),
                    }
                }
            };
            
            let path_obj = Path::new(&file_path);
            
            // Serve file
            if path_obj.is_file() {
                match crate::handlers::serve_file(&file_path) {
                    Ok(content) => {
                        let mut response = HttpResponse::ok();
                        response.set_header("Content-Type", get_content_type(&file_path));
                        response.set_body_bytes(content);
                        return response;
                    }
                    Err(_) => {
                        return error_response(404, error_path, "Not Found");
                    }
                }
            }
            
            // Handle directory
            if path_obj.is_dir() {
                if let Some(df) = &route.default_file {
                    let default_path = format!("{}/{}", file_path, df);
                    if let Ok(content) = crate::handlers::serve_file(&default_path) {
                        let mut response = HttpResponse::ok();
                        response.set_header("Content-Type", "text/html");
                        response.set_body_bytes(content);
                        return response;
                    }
                }
                
                if route.autoindex {
                    return crate::handlers::list_directory(&file_path, &request.path, &route.root);
                }
                
                return error_response(403, error_path, "Forbidden");
            }
            
            error_response(404, error_path, "Not Found")
        }
        None => error_response(404, error_path, "Not Found")
    }
}

fn find_vhost<'a>(request: &HttpRequest, vhosts: &'a [VHost]) -> Option<&'a VHost> {
    let host = request.headers.get("Host")
        .and_then(|h| h.split(':').next());
    
    vhosts.iter()
        .find(|v| Some(v.name.as_str()) == host)
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
            let mut response = HttpResponse::new(code, message);
            response.set_body(&format!("<h1>{} - {}</h1>", code, message));
            response
        }
    }
}

fn get_content_type(file_path: &str) -> &str {
    if file_path.ends_with(".html") { "text/html" }
    else if file_path.ends_with(".css") { "text/css" }
    else if file_path.ends_with(".js") { "application/javascript" }
    else if file_path.ends_with(".json") { "application/json" }
    else if file_path.ends_with(".png") { "image/png" }
    else if file_path.ends_with(".jpg") || file_path.ends_with(".jpeg") { "image/jpeg" }
    else if file_path.ends_with(".gif") { "image/gif" }
    else if file_path.ends_with(".txt") { "text/plain" }
    else { "application/octet-stream" }
}