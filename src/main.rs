#[cfg(unix)]
use localhost::config::parse_config_file;

#[cfg(unix)]
use localhost::network::server::Server;

#[cfg(unix)]
fn main() {
    let config = parse_config_file("server.conf")
        .expect("Failed to load config");
    
    let server = Server::new(config);
    
    if let Err(e) = server.run() {
        eprintln!("Server error: {}", e);
    }
}

#[cfg(not(unix))]
fn main() {
    eprintln!("This server only works on Unix/Linux systems!");
}