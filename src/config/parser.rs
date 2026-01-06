use super::types::{RouteConfig, ServerConfig,VHost};
use std::fs;
use std::io;

pub fn parse_config_file(path: &str) -> io::Result<ServerConfig> {
    let content = fs::read_to_string(path)?;
    parse_config_string(&content)
}

pub fn parse_config_string(content: &str) -> io::Result<ServerConfig> {
    let mut listen_address = String::new();
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

    for line in content.lines() {
        let line = line.trim();
        
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
                    // Save previous vhost
                    if let Some(vh) = current_vhost.take() {
                        vhosts.push(vh);
                    }
                    
                    let name = line.split_whitespace().nth(1)
                        .ok_or(io::Error::new(io::ErrorKind::InvalidData, "Missing vhost name"))?
                        .to_string();
                    
                    current_vhost = Some(VHost {
                        name,
                        error_path: error_path.clone(),  // Use global or default
                        routes: Vec::new(),
                    });
                    in_vhost = true;
                }
                else if line.starts_with("route") && line.ends_with('{') {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        current_route_path = parts[1].to_string();
                        context = ParsingContext::InsideRoute;
                    }
                }
                else if line.starts_with("listen") {
                    if let Some(value) = line.split('=').nth(1) {
                        listen_address = value.trim().to_string();
                    }
                }
                else if line.starts_with("client_body_size_limit") {
                    if let Some(value) = line.split('=').nth(1) {
                        if let Ok(size) = value.trim().parse::<usize>() {
                            client_body_size_limit = size;
                        }
                    }
                }
                else if line.starts_with("error_path") {
                    if let Some(value) = line.split('=').nth(1) {
                        let path = value.trim().to_string();
                        error_path = path.clone();
                        
                        // Update current vhost error_path if inside vhost
                        if let Some(vh) = &mut current_vhost {
                            vh.error_path = path;
                        }
                    }
                }
            }
            
            ParsingContext::InsideRoute => {
                if line == "}" {
                    let route = RouteConfig {
                        path: current_route_path.clone(),
                        methods: current_route_methods.clone(),
                        root: current_route_root.clone(),
                        default_file: if current_default_file.is_empty() { 
                            None 
                        } else { 
                            Some(current_default_file.clone()) 
                        },
                        autoindex: current_route_autoindex,
                    };
                    
                    // Add to vhost or global routes
                    if let Some(vh) = &mut current_vhost {
                        vh.routes.push(route);
                    } else {
                        routes.push(route);
                    }
                    
                    // Reset
                    current_route_path.clear();
                    current_route_methods.clear();
                    current_route_root.clear();
                    current_default_file.clear();
                    current_route_autoindex = false;
                    
                    context = ParsingContext::TopLevel;
                }
                else if line.starts_with("methods") {
                    if let Some(value) = line.split('=').nth(1) {
                        current_route_methods = value
                            .split(',')
                            .map(|m| m.trim().to_string())
                            .collect();
                    }
                }
                else if line.starts_with("default_file") {
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
                        current_route_autoindex = value.trim() == "on";
                    }
                }
            }
        }
    }
    
    // Save last vhost
    if let Some(vh) = current_vhost {
        vhosts.push(vh);
    }
    
    Ok(ServerConfig {
        listen_address,
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