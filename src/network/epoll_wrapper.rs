#![cfg(unix)]
use std::os::unix::io::RawFd;
use std::io;

/// Events we care about
pub struct SocketEvent {
    pub fd: RawFd,
    pub can_read: bool,
    pub can_write: bool,
    pub has_error: bool,
    pub hung_up: bool,
}

/// Interest flags for registration
#[derive(Clone, Copy)]
pub struct Interest {
    pub read: bool,
    pub write: bool,
}

impl Interest {
    pub fn readable() -> Self {
        Self { read: true, write: false }
    }
    
    pub fn writable() -> Self {
        Self { read: false, write: true }
    }
    
    pub fn both() -> Self {
        Self { read: true, write: true }
    }
}

/// Epoll wrapper for I/O multiplexing
pub struct Epoll {
    epoll_fd: RawFd,
}

impl Epoll {
    /// Create new epoll instance
    pub fn create() -> io::Result<Self> {
        let fd = unsafe { libc::epoll_create1(0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { epoll_fd: fd })
    }
    
    /// Register a file descriptor with specified interest
    pub fn register(&self, fd: RawFd, interest: Interest) -> io::Result<()> {
        let events = self.build_event_mask(interest);
        
        let mut ev = libc::epoll_event {
            events,
            u64: fd as u64,
        };
        
        let result = unsafe {
            libc::epoll_ctl(
                self.epoll_fd,
                libc::EPOLL_CTL_ADD,
                fd,
                &mut ev,
            )
        };
        
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    
    /// Modify interest for already registered fd
    pub fn modify(&self, fd: RawFd, interest: Interest) -> io::Result<()> {
        let events = self.build_event_mask(interest);
        
        let mut ev = libc::epoll_event {
            events,
            u64: fd as u64,
        };
        
        let result = unsafe {
            libc::epoll_ctl(
                self.epoll_fd,
                libc::EPOLL_CTL_MOD,
                fd,
                &mut ev,
            )
        };
        
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    
    /// Remove fd from epoll
    pub fn unregister(&self, fd: RawFd) -> io::Result<()> {
        let result = unsafe {
            libc::epoll_ctl(
                self.epoll_fd,
                libc::EPOLL_CTL_DEL,
                fd,
                std::ptr::null_mut(),
            )
        };
        
        if result < 0 {
            // Ignore ENOENT - fd might already be closed
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::ENOENT) {
                return Err(err);
            }
        }
        Ok(())
    }
    
    /// Wait for events with timeout (milliseconds, -1 for infinite)
    pub fn poll(&self, timeout_ms: i32) -> io::Result<Vec<SocketEvent>> {
        let mut raw_events = vec![libc::epoll_event { events: 0, u64: 0 }; 128];
        
        let count = unsafe {
            libc::epoll_wait(
                self.epoll_fd,
                raw_events.as_mut_ptr(),
                raw_events.len() as i32,
                timeout_ms,
            )
        };
        
        if count < 0 {
            let err = io::Error::last_os_error();
            // EINTR is not a real error - just interrupted
            if err.raw_os_error() == Some(libc::EINTR) {
                return Ok(Vec::new());
            }
            return Err(err);
        }
        
        let mut results = Vec::with_capacity(count as usize);
        for i in 0..count as usize {
            let ev = &raw_events[i];
            let flags = ev.events;
            
            results.push(SocketEvent {
                fd: ev.u64 as RawFd,
                can_read: (flags & libc::EPOLLIN as u32) != 0,
                can_write: (flags & libc::EPOLLOUT as u32) != 0,
                has_error: (flags & libc::EPOLLERR as u32) != 0,
                hung_up: (flags & libc::EPOLLHUP as u32) != 0,
            });
        }
        
        Ok(results)
    }
    
    /// Build event mask from interest flags
    fn build_event_mask(&self, interest: Interest) -> u32 {
        let mut mask: u32 = 0;
        
        if interest.read {
            mask |= libc::EPOLLIN as u32;
        }
        if interest.write {
            mask |= libc::EPOLLOUT as u32;
        }
        
        // Always watch for errors and hangups
        mask |= libc::EPOLLERR as u32;
        mask |= libc::EPOLLHUP as u32;
        
        mask
    }
}

impl Drop for Epoll {
    fn drop(&mut self) {
        unsafe { libc::close(self.epoll_fd) };
    }
}
