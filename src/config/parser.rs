use super::types::{RouteConfig, ServerConfig};
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
    let mut routes = Vec::new();  // ← NEW: Store routes here
    
    // Where are we in the file?
    let mut context = ParsingContext::TopLevel;
    
    // Temporary storage while building a route
    let mut current_route_path = String::new();
    let mut current_route_methods = Vec::new();
    let mut current_route_root = String::new();
    let mut current_default_file = String::new();
    
    for line in content.lines() {
        let line = line.trim();
        
        if line.is_empty() {
            continue;
        }
        
        // What do we do with this line?
        match context {
            ParsingContext::TopLevel => {
                // Are we starting a route?
                if line.starts_with("route ") && line.ends_with('{') {
                    // Extract the path
                    // "route /upload {" → "/upload"
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        current_route_path = parts[1].to_string();
                        context = ParsingContext::InsideRoute; 
                    }
                }
                // Is it a top-level setting?
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
                            error_path = value.trim().to_string();
                    }
                }
            }
            
            ParsingContext::InsideRoute => {
                // Are we closing the route?
                if line == "}" {
                    // Save the route we just built
                    routes.push(RouteConfig {
                        path: current_route_path.clone(),
                        methods: current_route_methods.clone(),
                        root: current_route_root.clone(),
                        default_file: Some(current_default_file.clone()),
                    });
                    
                    // Reset for next route
                    current_route_path.clear();
                    current_route_methods.clear();
                    current_route_root.clear();
                    current_default_file.clear();
                    
                    context = ParsingContext::TopLevel; // ← Change context back!
                }
                // Is it a route setting?
                else if line.starts_with("methods") {
                    if let Some(value) = line.split('=').nth(1) {
                        // "POST,DELETE" → ["POST", "DELETE"]
                        current_route_methods = value
                            .split(',')
                            .map(|m| m.trim().to_string())
                            .collect();
                    }
                }else if line.starts_with("default_file"){
                    if let Some(value)= line.split("=").nth(1){
                        current_default_file = value.trim().to_string();
                    }
                }
                else if line.starts_with("root") {
                    if let Some(value) = line.split('=').nth(1) {
                        current_route_root = value.trim().to_string();
                    }
                }
            }
        }
    }
    
    Ok(ServerConfig {
        listen_address,
        client_body_size_limit,
        routes,
        error_path,
    })
}

// The context tracker
enum ParsingContext {
    TopLevel,
    InsideRoute,
}