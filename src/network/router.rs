use crate::config::{RouteConfig, ServerConfig, VHost};
use crate::http::{HttpRequest, HttpResponse};

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
                
                // DEBUG: Print what we're computing
                eprintln!("[CGI DEBUG] request.path = '{}'", request.path);
                eprintln!("[CGI DEBUG] route.path = '{}'", route.path);
                eprintln!("[CGI DEBUG] route.root = '{}'", route.root);
                eprintln!("[CGI DEBUG] after_route = '{}'", after_route);
                
                // Handle empty script name
                if after_route.is_empty() {
                    return error_response(404, error_path, "Not Found");
                }
                
                // Build script path
                // If after_route already starts with route.root, don't duplicate it
                let script_path = if after_route.starts_with(&route.root) {
                    after_route.to_string()
                } else {
                    format!("{}/{}", route.root, after_route)
                };
                
                eprintln!("[CGI DEBUG] script_path = '{}'", script_path);
                
                let path_info = request.path.clone();
                return crate::handlers::run_cgi(&script_path, &path_info, request);
            }
            
            // Build file path
            let file_path = if route.path == "/" {
                let req_path = request.path.trim_start_matches('/');
                if req_path.is_empty() {
                    if let Some(df) = &route.default_file {
                        format!("{}/{}", route.root, df)
                    } else {
                        route.root.clone()
                    }
                } else {
                    format!("{}/{}", route.root, req_path)
                }
            } else {
                let after = request.path.strip_prefix(&route.path)
                    .unwrap_or("")
                    .trim_start_matches('/');
                
                if after.is_empty() {
                    route.root.clone()
                } else {
                    format!("{}/{}", route.root, after)
                }
            };
            
            let path_obj = std::path::Path::new(&file_path);
            
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