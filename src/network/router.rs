use crate::config::RouteConfig;
use crate::config::ServerConfig;
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
    configs: &ServerConfig,
) -> HttpResponse {
    let matched_route = find_route(request, &configs.routes);
    
    match matched_route {
        Some(route) => {
            if !route.methods.contains(&request.method) {                
                let error_file = format!("{}/405.html", configs.error_path);
                match crate::handlers::serve_file(&error_file) {
                    Ok(content) => {
                        let mut response = HttpResponse::method_not_allowed();
                        response.set_header("Content-Type", "text/html");
                        response.set_body_bytes(content);
                        return response;
                    }
                    Err(_) => {
                        let mut response = HttpResponse::method_not_allowed();
                        response.set_body("<h1>405 - Method Not Allowed</h1>");
                        return response;
                    }
                }
            }

            if route.path.contains("/upload"){
                if request.method.eq_ignore_ascii_case("POST"){
                    return crate::handlers::upload_file(request,configs.client_body_size_limit);
                } else if request.method.eq_ignore_ascii_case("DELETE"){
                    return crate::handlers::delete_file(request);
                }
            }
            
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

            if path_obj.is_file() {
                match crate::handlers::serve_file(&file_path) {
                    Ok(content) => {
                        let mut response = HttpResponse::ok();
                        response.set_header("Content-Type", get_content_type(&file_path));
                        response.set_body_bytes(content);
                        return response;
                    }
                    Err(_) => return HttpResponse::not_found(),
                }
            } else if path_obj.is_dir() {
                if route.autoindex {
                    return crate::handlers::list_directory(&file_path, &request.path, &route.root);
                }
            }

            HttpResponse::not_found()
        }
        None => {            
            let error_file = format!("{}/404.html", configs.error_path);
            match crate::handlers::serve_file(&error_file) {
                Ok(content) => {
                    let mut response = HttpResponse::not_found();
                    response.set_header("Content-Type", "text/html");
                    response.set_body_bytes(content);
                    response
                }
                Err(_) => {
                    let mut response = HttpResponse::not_found();
                    response.set_body("<h1>404 - Route Not Found</h1>");
                    response
                }
            }
        }
    }
}

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