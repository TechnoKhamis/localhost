#![cfg(unix)]
use std::collections::{HashMap, HashSet};
use crate::config::ServerConfig;
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::AsRawFd;
use std::os::fd::RawFd;
use super::{create_listener, route_request, Epoll};
use crate::http::{HttpRequest, HttpResponse};
use std::io::{Write, Read};

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
        // Create multiple listeners
        let mut listeners = Vec::new();
        let mut listener_fds = HashSet::new();
        
        for addr in &self.config.listen_addresses {
            match create_listener(addr) {
                Ok(listener) => {
                    let fd = listener.as_raw_fd();
                    listener_fds.insert(fd);
                    listeners.push(listener);
                    println!("[listen] bound {}", addr);
                }
                Err(e) => {
                    eprintln!("[listen] FAILED to bind {}: {}", addr, e);
                }
            }
        }
        
        if listeners.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "No valid listeners created"
            ));
        }
        
        // Create epoll and add all listeners
        let epoll = Epoll::new()?;
        for listener in &listeners {
            epoll.add(listener.as_raw_fd())?;
        }
        
        let mut connections: HashMap<RawFd, Connection> = HashMap::new();
        
        println!("Server running on: {}", self.config.listen_addresses.join(", "));
        
        loop {
            let events = epoll.wait(1000)?;
            
            for event in events {
                if listener_fds.contains(&event.fd) {
                    // Find the right listener
                    if let Some(listener) = listeners.iter().find(|l| l.as_raw_fd() == event.fd) {
                        self.accept_connection(listener, &epoll, &mut connections)?;
                    }
                } else if event.readable {
                    self.handle_read(event.fd, &mut connections)?;
                }
            }
        }
    }
    
    fn accept_connection(
        &self,
        listener: &TcpListener,
        epoll: &Epoll,
        connections: &mut HashMap<RawFd, Connection>,
    ) -> std::io::Result<()> {
        loop {
            match listener.accept() {
                Ok((stream, _addr)) => {
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
    
    conn.stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
    
    let mut temp = [0u8; 4096];
    loop {
        match conn.stream.read(&mut temp) {
            Ok(0) => {
                connections.remove(&fd);
                return Ok(());
            }
            Ok(n) => {
                conn.buffer.extend_from_slice(&temp[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // TIMEOUT OCCURRED
                println!("Connection {} timed out", fd);
                connections.remove(&fd);
                return Ok(());
            }
            Err(e) => {
                println!("Read error: {}", e);
                connections.remove(&fd);
                return Ok(());
            }
        }
    }
    
    if let Some(request) = HttpRequest::parse(&conn.buffer) {
        println!("Received: {} {}", request.method, request.path);
        
        let mut response = self.handle_request(&request);
        
        let keep_alive = request.headers.get("Connection")
            .map(|v| v.as_str() != "close")
            .unwrap_or(true);
        
        response.set_header("Connection", if keep_alive { "keep-alive" } else { "close" });
        
        let response_bytes = response.to_bytes();
        let _ = conn.stream.write_all(&response_bytes);
        
        if !keep_alive {
            connections.remove(&fd);
        } else {
            conn.buffer.clear();
        }
    }
    
    Ok(())
}

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