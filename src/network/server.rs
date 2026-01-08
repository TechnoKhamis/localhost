#![cfg(unix)]
use std::collections::{HashMap, HashSet};
use std::net::TcpListener;
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Duration;

use crate::config::ServerConfig;
use crate::http::{HttpRequest, HttpResponse};

use super::connection::{ClientConnection, ConnState, ConnectionError};
use super::epoll_wrapper::{Epoll, Interest};
use super::{create_listener, route_request};

/// Maximum idle time before closing connection
const IDLE_TIMEOUT_SECS: u64 = 30;

/// How often to check for timeouts (milliseconds)
const TIMEOUT_CHECK_MS: i32 = 1000;

/// Maximum requests per keep-alive connection
const MAX_REQUESTS_PER_CONN: u32 = 100;

pub struct Server {
    config: ServerConfig,
}

impl Server {
    pub fn new(config: ServerConfig) -> Self {
        Self { config }
    }
    
    pub fn run(&self) -> std::io::Result<()> {
        // Create listeners for all configured addresses
        let mut listeners: Vec<TcpListener> = Vec::new();
        let mut listener_fds: HashSet<RawFd> = HashSet::new();
        
        for addr in &self.config.listen_addresses {
            match create_listener(addr) {
                Ok(listener) => {
                    let fd = listener.as_raw_fd();
                    listener_fds.insert(fd);
                    listeners.push(listener);
                    println!("[server] listening on {}", addr);
                }
                Err(e) => {
                    eprintln!("[server] failed to bind {}: {}", addr, e);
                }
            }
        }
        
        if listeners.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "No listeners created"
            ));
        }
        
        // Create epoll instance
        let poller = Epoll::create()?;
        
        // Register all listeners for read events (incoming connections)
        for listener in &listeners {
            poller.register(listener.as_raw_fd(), Interest::readable())?;
        }
        
        // Track all client connections
        let mut clients: HashMap<RawFd, ClientConnection> = HashMap::new();
        
        println!("[server] ready to accept connections");
        
        // Main event loop
        loop {
            // Wait for events (with timeout for cleanup)
            let events = poller.poll(TIMEOUT_CHECK_MS)?;
            
            // Process each event
            for event in &events {
                let fd = event.fd;
                
                // Check if it's a listener socket
                if listener_fds.contains(&fd) {
                    // Accept new connections
                    if let Some(listener) = listeners.iter().find(|l| l.as_raw_fd() == fd) {
                        self.accept_connections(listener, &poller, &mut clients)?;
                    }
                    continue;
                }
                
                // Handle client socket events
                if let Some(client) = clients.get_mut(&fd) {
                    let mut should_close = false;
                    let mut needs_interest_update = false;
                    
                    // Handle errors/hangups first
                    if event.has_error || event.hung_up {
                        should_close = true;
                    }
                    
                    // Handle readable event
                    if event.can_read && !should_close {
                        match self.handle_read(client) {
                            Ok(request_ready) => {
                                if request_ready {
                                    // Process request and queue response
                                    self.process_and_queue_response(client);
                                    needs_interest_update = true;
                                }
                            }
                            Err(_) => {
                                should_close = true;
                            }
                        }
                    }
                    
                    // Handle writable event
                    if event.can_write && !should_close {
                        match self.handle_write(client) {
                            Ok(write_done) => {
                                if write_done {
                                    // Response sent completely
                                    if client.keep_alive && 
                                       client.requests_handled < MAX_REQUESTS_PER_CONN {
                                        // Reset for next request
                                        client.reset_for_next_request();
                                        needs_interest_update = true;
                                    } else {
                                        should_close = true;
                                    }
                                }
                            }
                            Err(_) => {
                                should_close = true;
                            }
                        }
                    }
                    
                    // Update epoll interest if state changed
                    if needs_interest_update && !should_close {
                        let interest = self.get_interest_for_state(client.state);
                        if let Err(e) = poller.modify(fd, interest) {
                            eprintln!("[server] epoll modify failed: {}", e);
                            should_close = true;
                        }
                    }
                    
                    // Mark for closing if needed
                    if should_close {
                        client.state = ConnState::Closing;
                    }
                }
            }
            
            // Clean up closed/timed-out connections
            self.cleanup_connections(&poller, &mut clients);
        }
    }
    
    /// Accept all pending connections from a listener
    fn accept_connections(
        &self,
        listener: &TcpListener,
        poller: &Epoll,
        clients: &mut HashMap<RawFd, ClientConnection>,
    ) -> std::io::Result<()> {
        loop {
            match listener.accept() {
                Ok((stream, addr)) => {
                    // Set non-blocking
                    stream.set_nonblocking(true)?;
                    
                    let fd = stream.as_raw_fd();
                    
                    // Register with epoll for read events initially
                    if let Err(e) = poller.register(fd, Interest::readable()) {
                        eprintln!("[server] failed to register client: {}", e);
                        continue;
                    }
                    
                    // Create connection state
                    let conn = ClientConnection::new(stream);
                    clients.insert(fd, conn);
                    
                    println!("[server] accepted connection from {}", addr);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No more pending connections
                    break;
                }
                Err(e) => {
                    eprintln!("[server] accept error: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }
    
    /// Handle read event - returns true if complete request is ready
    fn handle_read(&self, client: &mut ClientConnection) -> Result<bool, ConnectionError> {
        // Only one read per event!
        let bytes = client.try_read()?;
        
        if bytes == 0 {
            return Ok(false);
        }
        
        // Check if we have a complete request
        Ok(HttpRequest::parse(&client.read_buffer).is_some())
    }
    
    /// Handle write event - returns true if write is complete
    fn handle_write(&self, client: &mut ClientConnection) -> Result<bool, ConnectionError> {
        // Only one write per event!
        client.try_write()
    }
    
    /// Process request and queue response for writing
    fn process_and_queue_response(&self, client: &mut ClientConnection) {
        // Parse request (we know it's complete)
        let request = match HttpRequest::parse(&client.read_buffer) {
            Some(req) => req,
            None => {
                // Shouldn't happen, but handle gracefully
                let resp = HttpResponse::bad_request();
                client.queue_response(resp.to_bytes());
                client.keep_alive = false;
                return;
            }
        };
        
        println!("[server] {} {}", request.method, request.path);
        
        // Check Connection header
        client.keep_alive = request.headers
            .get("Connection")
            .map(|v| !v.eq_ignore_ascii_case("close"))
            .unwrap_or(true);
        
        // Route and generate response
        let mut response = route_request(&request, &self.config);
        
        // Handle session cookie
        let cookie = request.headers.get("Cookie").map(|s| s.as_str());
        if crate::handlers::get_session_id(cookie).is_none() {
            let sid = crate::handlers::create_session_id();
            response.set_header("Set-Cookie", &format!("SID={}; Path=/; HttpOnly", sid));
        }
        
        // Set connection header
        if client.keep_alive {
            response.set_header("Connection", "keep-alive");
            response.set_header("Keep-Alive", "timeout=30, max=100");
        } else {
            response.set_header("Connection", "close");
        }
        
        // Queue response for writing
        let response_bytes = response.to_bytes();
        client.queue_response(response_bytes);
    }
    
    /// Get epoll interest flags based on connection state
    fn get_interest_for_state(&self, state: ConnState) -> Interest {
        match state {
            ConnState::Reading => Interest::readable(),
            ConnState::Writing => Interest::writable(),
            ConnState::Closing => Interest::readable(), // Will be removed anyway
        }
    }
    
    /// Clean up closed and timed-out connections
    fn cleanup_connections(
        &self,
        poller: &Epoll,
        clients: &mut HashMap<RawFd, ClientConnection>,
    ) {
        let fds_to_remove: Vec<RawFd> = clients
            .iter()
            .filter(|(_, conn)| {
                conn.state == ConnState::Closing ||
                conn.is_timed_out(IDLE_TIMEOUT_SECS)
            })
            .map(|(fd, _)| *fd)
            .collect();
        
        for fd in fds_to_remove {
            if let Some(conn) = clients.remove(&fd) {
                // Unregister from epoll
                let _ = poller.unregister(fd);
                
                // Shutdown socket gracefully
                let _ = conn.stream.shutdown(std::net::Shutdown::Both);
                
                if conn.is_timed_out(IDLE_TIMEOUT_SECS) {
                    println!("[server] connection timed out");
                } else {
                   // println!("[server] connection closed");
                }
            }
        }
    }
}
