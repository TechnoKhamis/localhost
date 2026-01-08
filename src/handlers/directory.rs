use crate::http::HttpResponse;

pub fn list_directory(path: &str, uri: &str, _route: &str) -> HttpResponse {
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return HttpResponse::not_found(),
    };
    
    let mut html = format!("<h1>Index of {}</h1><ul>", uri);
    
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        
        let link = if uri == "/" {
            format!("/{}", name)
        } else {
            format!("{}{}", uri, name)
        };
        
        html.push_str(&format!(r#"<li><a href="{}">{}</a></li>"#, link, name));
    }
    
    html.push_str("</ul>");
    
    let mut resp = HttpResponse::ok();
    resp.set_header("Content-Type", "text/html");
    resp.set_body(&html);
    resp
}
