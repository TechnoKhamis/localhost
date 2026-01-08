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
    let mut current_redirect:Option<String>= None;

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        
        // Skip comments and strip inline comments
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
                    // Save previous vhost
                    if let Some(vh) = current_vhost.take() {
                        vhosts.push(vh);
                    }
                    
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 3 || parts[2] != "{" {
                        eprintln!("Warning: Invalid vhost syntax at line {}: {}", line_num + 1, line);
                        continue;
                    }
                    
                    let name = parts[1].to_string();
                    current_vhost = Some(VHost {
                        name,
                        error_path: error_path.clone(),
                        routes: Vec::new(),
                    });
                    in_vhost = true;
                }
                else if line.starts_with("route") && line.ends_with('{') {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() < 3 || parts[2] != "{" {
                        eprintln!("Warning: Invalid route syntax at line {}: {}", line_num + 1, line);
                        continue;
                    }
                    current_route_path = parts[1].to_string();
                    context = ParsingContext::InsideRoute;
                }
                else if line.starts_with("listen") {
                    if let Some(value) = line.split('=').nth(1) {
                        let value = value.trim();
                        if value.is_empty() {
                            eprintln!("Warning: Empty listen address at line {}", line_num + 1);
                            continue;
                        }
                        for addr in value.split(',') {
                            let trimmed = addr.trim();
                            if !trimmed.is_empty() {
                                if trimmed.contains(':') {
                                    // Check for duplicate
                                    if listen_addresses.contains(&trimmed.to_string()) {
                                        eprintln!("Warning: Duplicate listen address '{}' ignored at line {}", trimmed, line_num + 1);
                                    } else {
                                        listen_addresses.push(trimmed.to_string());
                                    }
                                } else {
                                    eprintln!("Warning: Invalid address format '{}' at line {}", trimmed, line_num + 1);
                                }
                            }
                        }
                    }
                }
                else if line.starts_with("client_body_size_limit") {
                    if let Some(value) = line.split('=').nth(1) {
                        match value.trim().parse::<usize>() {
                            Ok(size) => client_body_size_limit = size,
                            Err(_) => eprintln!("Warning: Invalid body size limit at line {}: {}", line_num + 1, value),
                        }
                    } else {
                        eprintln!("Warning: Invalid client_body_size_limit syntax at line {}", line_num + 1);
                    }
                }
                else if line.starts_with("error_path") {
                    if let Some(value) = line.split('=').nth(1) {
                        let path = value.trim();
                        if !path.is_empty() {
                            error_path = path.to_string();
                            
                            // Update current vhost error_path if inside vhost
                            if let Some(vh) = &mut current_vhost {
                                vh.error_path = path.to_string();
                            }
                        } else {
                            eprintln!("Warning: Empty error_path at line {}", line_num + 1);
                        }
                    }
                }
                else {
                    eprintln!("Warning: Unknown directive '{}' at line {}", line, line_num + 1);
                }
            }
            
            ParsingContext::InsideRoute => {
                if line == "}" {
                    // Validate route before adding
                    if current_route_root.is_empty() && current_redirect.is_none(){
                        eprintln!("Warning: Route '{}' missing root, skipping", current_route_path);
                    } else if current_route_methods.is_empty() {
                        eprintln!("Warning: Route '{}' missing methods, skipping", current_route_path);
                    } else {
                        let route = RouteConfig {
                            path: current_route_path.clone(),
                            methods: current_route_methods.clone(),
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
                        
                        // Add to vhost or global routes
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
                        let methods: Vec<String> = value
                            .split(',')
                            .map(|m| m.trim().to_uppercase())
                            .filter(|m| !m.is_empty())
                            .collect();
                        
                        if methods.is_empty() {
                            eprintln!("Warning: Empty methods list at line {}", line_num + 1);
                        } else {
                            current_route_methods = methods;
                        }
                    } else {
                        eprintln!("Warning: Invalid methods syntax at line {}", line_num + 1);
                    }
                }
                else if line.starts_with("default_file") {
                    if let Some(value) = line.split('=').nth(1) {
                        let val = value.trim();
                        if !val.is_empty() {
                            current_default_file = val.to_string();
                        } else {
                            eprintln!("Warning: Empty default_file at line {}", line_num + 1);
                        }
                    }
                }
                else if line.starts_with("root") {
                    if let Some(value) = line.split('=').nth(1) {
                        let val = value.trim();
                        if !val.is_empty() {
                            current_route_root = val.to_string();
                        } else {
                            eprintln!("Warning: Empty root at line {}", line_num + 1);
                        }
                    }
                }
                else if line.starts_with("autoindex") {
                    if let Some(value) = line.split('=').nth(1) {
                        let val = value.trim().to_lowercase();
                        current_route_autoindex = val == "on" || val == "true" || val == "yes";
                    } else {
                        eprintln!("Warning: Invalid autoindex syntax at line {}", line_num + 1);
                    }
                }
                else if line.starts_with("cgi") {
                    if let Some(value) = line.split('=').nth(1) {
                        let val = value.trim();
                        if !val.is_empty() {
                            current_cgi = Some(val.to_string());
                        } else {
                            eprintln!("Warning: Empty cgi value at line {}", line_num + 1);
                        }
                    }
                }
                else if line.starts_with("redirect") {
                    if let Some(value) = line.split('=').nth(1) {
                        current_redirect = Some(value.trim().to_string());
                    }
                }
                else {
                    eprintln!("Warning: Unknown route directive '{}' at line {}", line, line_num + 1);
                }
            }
        }
    }
    
    // Save last vhost if any
    if let Some(vh) = current_vhost {
        vhosts.push(vh);
    }
    
    // Validate final configuration
    if listen_addresses.is_empty() {
        eprintln!("Warning: No listen addresses configured, using default 127.0.0.1:8080");
        listen_addresses.push("127.0.0.1:8080".to_string());
    }
    
    // Validate vhosts have routes
    for vhost in &vhosts {
        if vhost.routes.is_empty() {
            eprintln!("Warning: Vhost '{}' has no routes", vhost.name);
        }
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