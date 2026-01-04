#![cfg(unix)]
use std::os::unix::io::RawFd;
use std::io;

/// Epoll event wrapper
pub struct EpollEvent {
    pub fd: RawFd,
    pub readable: bool,
    pub writable: bool,
}

/// Epoll wrapper for monitoring multiple file descriptors
pub struct Epoll {
    epoll_fd: RawFd,
}

impl Epoll {
    /// Create new epoll
    pub fn new() -> io::Result<Self> {
        // Call Linux system call to create epoll
        let epoll_fd = unsafe { libc::epoll_create1(0) };
        
        // Check if it failed
        if epoll_fd < 0 {
            return Err(io::Error::last_os_error());
        }
        
        Ok(Self { epoll_fd })
    }
    
    
    /// Tell epoll to watch a socket
    pub fn add(&self, fd: RawFd) -> io::Result<()> {
        // Create event structure
        let mut event = libc::epoll_event {
            events: libc::EPOLLIN as u32,  // Watch for incoming data
            u64: fd as u64,                // Store the fd
        };
        
        // Add to epoll
        let result = unsafe {
            libc::epoll_ctl(
                self.epoll_fd,           // Our epoll instance
                libc::EPOLL_CTL_ADD,     // ADD operation
                fd,                      // Socket to watch
                &mut event,              // Event config
            )
        };
        
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        
        Ok(())
    }

    // Wait for events (with timeout in milliseconds)
    pub fn wait(&self, timeout_ms: i32) -> io::Result<Vec<EpollEvent>> {
        let mut events = vec![libc::epoll_event { events: 0, u64: 0 }; 64];
        
        let count = unsafe {
            libc::epoll_wait(
                self.epoll_fd,
                events.as_mut_ptr(),
                events.len() as i32,
                timeout_ms,
            )
        };
        
        if count < 0 {
            return Err(io::Error::last_os_error());
        }
        
        let mut result = Vec::new();
        for i in 0..count as usize {
            let event = &events[i];
            result.push(EpollEvent {
                fd: event.u64 as RawFd,
                readable: (event.events & libc::EPOLLIN as u32) != 0,
                writable: (event.events & libc::EPOLLOUT as u32) != 0,
            });
        }
        
        Ok(result)
    }
}

impl Drop for Epoll {
    fn drop(&mut self) {
        unsafe { libc::close(self.epoll_fd) };
    }
}