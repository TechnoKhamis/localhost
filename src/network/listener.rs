#![cfg(unix)]
use std::net::TcpListener;
use std::os::unix::io::{AsRawFd, RawFd};
use std::io;

/// Set socket to non-blocking mode
pub fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    
    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    
    Ok(())
}

/// Create non-blocking TCP listener
pub fn create_listener(address: &str) -> io::Result<TcpListener> {
    let listener = TcpListener::bind(address)?;
    set_nonblocking(listener.as_raw_fd())?;
    Ok(listener)
}
