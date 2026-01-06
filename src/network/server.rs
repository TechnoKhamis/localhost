#![cfg(unix)]
use std::collections::HashMap;
use crate::config::ServerConfig;
use std::net::TcpStream;
use std::os::unix::io::AsRawFd;
use std::os::fd::RawFd;
use crate::network::router;
use super::{create_listener,route_request, Epoll};
use crate::http::HttpRequest;
use crate::http::HttpResponse;
 use std::io::Write;
  use std::io::Read;

struct Connection {
    stream: TcpStream,
    buffer: Vec<u8>,
}


pub struct Server {
    config: ServerConfig,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }
    
    pub fn run(&self) -> std::io::Result<()> {

        //Create routing table

        
        println!("Starting server on {}", self.config.listen_address);
        
        // Create listener
        let listener = create_listener(&self.config.listen_address)?;
        println!("Listening on {}", self.config.listen_address);
        
        // Create epoll
        let epoll = Epoll::new()?;
        epoll.add(listener.as_raw_fd())?;
        
        let listener_fd = listener.as_raw_fd();
        let mut connections: HashMap<RawFd, Connection> = HashMap::new();
        
        loop {
            // Wait for events
            let events = epoll.wait(1000)?;
            
            for event in events {
                if event.fd == listener_fd {
                    // New connection
                    self.accept_connection(&listener, &epoll, &mut connections)?;
                } else if event.readable {
                    // Data ready to read
                    self.handle_read(event.fd, &mut connections)?;
                }
            }
        }
    }
    
       fn accept_connection(
        &self,
        listener: &std::net::TcpListener,
        epoll: &Epoll,
        connections: &mut HashMap<RawFd, Connection>,
    ) -> std::io::Result<()> {
        loop {
            match listener.accept() {
                Ok((stream, addr)) => {
                    
                    let fd = stream.as_raw_fd();
                    super::set_nonblocking(fd)?;
                    epoll.add(fd)?;
                    
                    connections.insert(fd, Connection {
                        stream,
                        buffer: Vec::new(),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }


      fn handle_read(
        &self,
        fd: RawFd,
        connections: &mut HashMap<RawFd, Connection>,
    ) -> std::io::Result<()> {
        let conn = match connections.get_mut(&fd) {
            Some(c) => c,
            None => return Ok(()),
        };
        
        // Read data
        let mut temp = [0u8; 4096];
        loop {
            match conn.stream.read(&mut temp) {
                Ok(0) => {
                    // Connection closed
                    println!("Connection closed");
                    connections.remove(&fd);
                    return Ok(());
                }
                Ok(n) => {
                    conn.buffer.extend_from_slice(&temp[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    println!("Read error: {}", e);
                    connections.remove(&fd);
                    return Ok(());
                }
            }
        }
        
        // Try to parse request
        if let Some(request) = HttpRequest::parse(&conn.buffer) {
            println!("Received: {} {}", request.method, request.path);
            
            // Build response
           let response = self.handle_request(&request);


           let response_byte = response.to_bytes();

           &conn.stream.write_all(&response_byte)?;
            
            
            // Close connection (for now)
            connections.remove(&fd);
        }
        
        Ok(())
    }

    /// Handle HTTP request and build response
fn handle_request(&self, request: &HttpRequest) -> HttpResponse {

   let mut response = route_request(&request, &self.config);
    let cookie = request.headers.get("Cookie").map(|s| s.as_str());
    if crate::handlers::get_session_id(cookie).is_none() {
        let sid = crate::handlers::create_session_id();
        response.set_header("Set-Cookie", &format!("SID={}; Path=/; HttpOnly", sid));
    }
    response
}


}