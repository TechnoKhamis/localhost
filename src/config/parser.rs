use super::types::{RouteConfig, ServerConfig, VHost};
use std::fs;
use std::io;

pub fn parse_config_file(path: &str) -> io::Result<ServerConfig> {
    let content = fs::read_to_string(path)?;
    parse_config_string(&content)
}

pub fn parse_config_string(content: &str) -> io::Result<ServerConfig> {
    let mut listen_addresses = Vec::new();
    let mut client_body_size_limit = 10 * 1024 * 1024;
    let mut error_path = String::new();
    let mut routes = Vec::new();
    let mut vhosts = Vec::new();
    
    let mut context = ParsingContext::TopLevel;
    let mut in_vhost = false;
    
    let mut current_route_path = String::new();
    let mut current_route_methods = Vec::new();
    let mut current_route_root = String::new();
    let mut current_default_file = String::new();
    let mut current_route_autoindex = false;
    let mut current_vhost: Option<VHost> = None;
    let mut current_cgi: Option<String> = None;
    let mut current_redirect: Option<String> = None;

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        
        // Skip comments
        let line = if let Some(pos) = line.find('#') {
            line[..pos].trim()
        } else {
            line
        };
        
        if line.is_empty() {
            continue;
        }
        
        // Close vhost block
        if line == "}" && in_vhost && context == ParsingContext::TopLevel {
            if let Some(vh) = current_vhost.take() {
                vhosts.push(vh);
            }
            in_vhost = false;
            continue;
        }
        
        match context {
            ParsingContext::TopLevel => {
                if line.starts_with("vhost") && line.ends_with('{') {
                    if let Some(vh) = current_vhost.take() {
                        vhosts.push(vh);
                    }
                    
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 3 || parts[2] != "{" {
                        continue;
                    }
                    
                    current_vhost = Some(VHost {
                        name: parts[1].to_string(),
                        error_path: error_path.clone(),
                        routes: Vec::new(),
                    });
                    in_vhost = true;
                }
                else if line.starts_with("route") && line.ends_with('{') {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 && parts[2] == "{" {
                        current_route_path = parts[1].to_string();
                        context = ParsingContext::InsideRoute;
                    }
                }
                else if line.starts_with("listen") {
                    if let Some(value) = line.split('=').nth(1) {
                        for addr in value.split(',') {
                            let trimmed = addr.trim();
                            if !trimmed.is_empty() && trimmed.contains(':') {
                                if listen_addresses.contains(&trimmed.to_string()) {
                                    // Report duplicate port configuration error
                                    eprintln!("[config] ERROR: duplicate listen address detected: {}", trimmed);
                                } else {
                                    listen_addresses.push(trimmed.to_string());
                                }
                            }
                        }
                    }
                }
                else if line.starts_with("client_body_size_limit") || line.starts_with("client_max_body_size") {
                    if let Some(value) = line.split('=').nth(1) {
                        if let Ok(size) = value.trim().parse::<usize>() {
                            client_body_size_limit = size;
                        }
                    }
                }
                else if line.starts_with("error_path") || line.starts_with("error_dir") {
                    if let Some(value) = line.split('=').nth(1) {
                        error_path = value.trim().to_string();
                        if let Some(vh) = &mut current_vhost {
                            vh.error_path = error_path.clone();
                        }
                    }
                }
            }
            
            ParsingContext::InsideRoute => {
                if line == "}" {
                    if !current_route_root.is_empty() || current_redirect.is_some() {
                        let route = RouteConfig {
                            path: current_route_path.clone(),
                            methods: if current_route_methods.is_empty() {
                                vec!["GET".to_string()]
                            } else {
                                current_route_methods.clone()
                            },
                            root: current_route_root.clone(),
                            default_file: if current_default_file.is_empty() {
                                Some("index.html".to_string())
                            } else {
                                Some(current_default_file.clone())
                            },
                            autoindex: current_route_autoindex,
                            cgi: current_cgi.clone(),
                            redirect: current_redirect.clone(),
                        };
                        
                        if let Some(vh) = &mut current_vhost {
                            vh.routes.push(route);
                        } else {
                            routes.push(route);
                        }
                    }
                    
                    // Reset
                    current_route_path.clear();
                    current_route_methods.clear();
                    current_route_root.clear();
                    current_default_file.clear();
                    current_route_autoindex = false;
                    current_cgi = None;
                    current_redirect = None;
                    context = ParsingContext::TopLevel;
                }
                else if line.starts_with("methods") {
                    if let Some(value) = line.split('=').nth(1) {
                        current_route_methods = value
                            .split(',')
                            .map(|m| m.trim().to_uppercase())
                            .filter(|m| !m.is_empty())
                            .collect();
                    }
                }
                else if line.starts_with("default_file") || line.starts_with("default") {
                    if let Some(value) = line.split('=').nth(1) {
                        current_default_file = value.trim().to_string();
                    }
                }
                else if line.starts_with("root") {
                    if let Some(value) = line.split('=').nth(1) {
                        current_route_root = value.trim().to_string();
                    }
                }
                else if line.starts_with("autoindex") {
                    if let Some(value) = line.split('=').nth(1) {
                        let val = value.trim().to_lowercase();
                        current_route_autoindex = val == "on" || val == "true" || val == "yes";
                    }
                }
                else if line.starts_with("cgi") {
                    if let Some(value) = line.split('=').nth(1) {
                        current_cgi = Some(value.trim().to_string());
                    }
                }
                else if line.starts_with("redirect") {
                    if let Some(value) = line.split('=').nth(1) {
                        current_redirect = Some(value.trim().to_string());
                    }
                }
            }
        }
    }
    
    if let Some(vh) = current_vhost {
        vhosts.push(vh);
    }
    
    if listen_addresses.is_empty() {
        listen_addresses.push("127.0.0.1:8080".to_string());
    }
    
    Ok(ServerConfig {
        listen_addresses,
        client_body_size_limit,
        routes,
        error_path,
        vhosts,
    })
}

#[derive(PartialEq)]
enum ParsingContext {
    TopLevel,
    InsideRoute,
}
