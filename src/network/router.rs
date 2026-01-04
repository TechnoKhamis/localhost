use crate::config::RouteConfig;
use crate::http::{HttpRequest, HttpResponse};

/// Find matching route for the request
pub fn find_route<'a>(
    request: &HttpRequest,
    routes: &'a [RouteConfig],
) -> Option<&'a RouteConfig> {
    routes
        .iter()
        .find(|route| route.path == request.path)
}

/// Route the request and return appropriate response
pub fn route_request(
    request: &HttpRequest,
    routes: &[RouteConfig],
    error_path: &str  // ⭐ Changed from &String
) -> HttpResponse {
    let matched_route = find_route(request, routes);
    
    match matched_route {
        Some(route) => {
            // Check if method is allowed
            if !route.methods.contains(&request.method) {                
                // Try to serve custom 405 page
                let error_file = format!("{}/405.html", error_path);
                match crate::handlers::serve_file(&error_file) {
                    Ok(content) => {
                        let mut response = HttpResponse::method_not_allowed();
                        response.set_header("Content-Type", "text/html");
                        response.set_body_bytes(content);
                        return response;
                    }
                    Err(_) => {
                        // Fallback to default message
                        let mut response = HttpResponse::method_not_allowed();
                        response.set_body("<h1>405 - Method Not Allowed</h1>");
                        return response;
                    }
                }
            }
            
            
            // Build file path
            let file_path = if let Some(df) = &route.default_file {
                format!("{}/{}", route.root, df)
            } else {
                route.root.clone()
            };
            
            println!("📂 Serving: {}", file_path);
            
            // Try to serve file
            match crate::handlers::serve_file(&file_path) {
                Ok(content) => {
                    let mut response = HttpResponse::ok();
                    response.set_header("Content-Type", get_content_type(&file_path));
                    response.set_body_bytes(content);
                    response
                }
                Err(error_msg) => {
                    let mut response = HttpResponse::not_found();
                    response.set_body(&format!("<h1>404 - {}</h1>", error_msg));
                    response
                }
            }
        }
        None => {            
            // Try to serve custom 404 page
            let error_file = format!("{}/404.html", error_path);
            match crate::handlers::serve_file(&error_file) {
                Ok(content) => {
                    let mut response = HttpResponse::not_found();
                    response.set_header("Content-Type", "text/html");
                    response.set_body_bytes(content);
                    response
                }
                Err(_) => {
                    // Fallback to default message
                    let mut response = HttpResponse::not_found();
                    response.set_body("<h1>404 - Route Not Found</h1>");
                    response
                }
            }
        }
    }
}

/// Get content type based on file extension
fn get_content_type(file_path: &str) -> &str {
    if file_path.ends_with(".html") {
        "text/html"
    } else if file_path.ends_with(".css") {
        "text/css"
    } else if file_path.ends_with(".js") {
        "application/javascript"
    } else if file_path.ends_with(".json") {
        "application/json"
    } else if file_path.ends_with(".png") {
        "image/png"
    } else if file_path.ends_with(".jpg") || file_path.ends_with(".jpeg") {
        "image/jpeg"
    } else if file_path.ends_with(".gif") {
        "image/gif"
    } else if file_path.ends_with(".svg") {
        "image/svg+xml"
    } else if file_path.ends_with(".txt") {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}