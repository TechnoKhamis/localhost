#![cfg(unix)]
use std::net::TcpStream;
use std::time::Instant;
use std::io::{Read, Write};

/// Connection state machine
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnState {
    /// Waiting to read request
    Reading,
    /// Have response, waiting to write
    Writing,
    /// Done, should close
    Closing,
}

/// Manages a single client connection
pub struct ClientConnection {
    pub stream: TcpStream,
    pub state: ConnState,
    
    // Read side
    pub read_buffer: Vec<u8>,
    
    // Write side  
    pub write_buffer: Vec<u8>,
    pub bytes_written: usize,
    
    // Timing
    pub connected_at: Instant,
    pub last_activity: Instant,
    
    // REQUEST TIMEOUT: Track when we started receiving this request
    pub request_started_at: Option<Instant>,
    
    // Keep-alive
    pub keep_alive: bool,
    pub requests_handled: u32,
}

impl ClientConnection {
    pub fn new(stream: TcpStream) -> Self {
        let now = Instant::now();
        Self {
            stream,
            state: ConnState::Reading,
            read_buffer: Vec::with_capacity(4096),
            write_buffer: Vec::new(),
            bytes_written: 0,
            connected_at: now,
            last_activity: now,
            request_started_at: None,  // No request started yet
            keep_alive: true,
            requests_handled: 0,
        }
    }
    
    /// Try to read data from socket (non-blocking)
    /// Returns: Ok(bytes_read), Err if connection should close
    pub fn try_read(&mut self) -> Result<usize, ConnectionError> {
        let mut temp = [0u8; 4096];
        
        match self.stream.read(&mut temp) {
            Ok(0) => {
                // Client closed connection
                Err(ConnectionError::Closed)
            }
            Ok(n) => {
                self.read_buffer.extend_from_slice(&temp[..n]);
                self.last_activity = Instant::now();
                
                // START REQUEST TIMEOUT: Mark when we first receive data for this request
                if self.request_started_at.is_none() {
                    self.request_started_at = Some(Instant::now());
                }
                
                Ok(n)
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available right now - that's fine
                Ok(0)
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                // Interrupted by signal, try again
                Ok(0)
            }
            Err(_) => {
                // Real error
                Err(ConnectionError::IoError)
            }
        }
    }
    
    /// Try to write data to socket (non-blocking)
    /// Returns: Ok(true) if all data written, Ok(false) if more to write
    pub fn try_write(&mut self) -> Result<bool, ConnectionError> {
        if self.bytes_written >= self.write_buffer.len() {
            // Nothing to write
            return Ok(true);
        }
        
        let remaining = &self.write_buffer[self.bytes_written..];
        
        match self.stream.write(remaining) {
            Ok(0) => {
                // Couldn't write anything - unusual but not error
                Ok(false)
            }
            Ok(n) => {
                self.bytes_written += n;
                self.last_activity = Instant::now();
                
                // Check if we're done
                let complete = self.bytes_written >= self.write_buffer.len();
                Ok(complete)
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Socket buffer full, try again later
                Ok(false)
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                // Interrupted, try again
                Ok(false)
            }
            Err(_) => {
                Err(ConnectionError::IoError)
            }
        }
    }
    
    /// Queue response for writing
    pub fn queue_response(&mut self, data: Vec<u8>) {
        self.write_buffer = data;
        self.bytes_written = 0;
        self.state = ConnState::Writing;
    }
    
    /// Reset for next request (keep-alive)
    pub fn reset_for_next_request(&mut self) {
        self.read_buffer.clear();
        self.write_buffer.clear();
        self.bytes_written = 0;
        self.state = ConnState::Reading;
        self.requests_handled += 1;
        // IMPORTANT: Reset request timeout for next request
        self.request_started_at = None;
    }
    
    /// Check if connection has been idle too long (no activity at all)
    pub fn is_idle_timeout(&self, timeout_secs: u64) -> bool {
        self.last_activity.elapsed().as_secs() > timeout_secs
    }
    
    /// Check if current REQUEST is taking too long (incomplete request timeout)
    pub fn is_request_timeout(&self, timeout_secs: u64) -> bool {
        if let Some(started) = self.request_started_at {
            // Only timeout if we're still reading (incomplete request)
            if self.state == ConnState::Reading {
                return started.elapsed().as_secs() > timeout_secs;
            }
        }
        false
    }
    
    /// Check if write is complete
    pub fn write_complete(&self) -> bool {
        self.bytes_written >= self.write_buffer.len()
    }
    
    /// Check if we want to read
    pub fn wants_read(&self) -> bool {
        self.state == ConnState::Reading
    }
    
    /// Check if we want to write
    pub fn wants_write(&self) -> bool {
        self.state == ConnState::Writing && !self.write_complete()
    }
}

#[derive(Debug)]
pub enum ConnectionError {
    Closed,
    IoError,
    Timeout,
}
