#![cfg(unix)]
use crate::http::{HttpRequest, HttpResponse};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Run a CGI script with NON-BLOCKING epoll-based I/O
pub fn run_cgi(script_path: &str, path_info: &str, request: &HttpRequest) -> HttpResponse {
    let script = Path::new(script_path);
    
    if !script.exists() {
        let mut resp = HttpResponse::not_found();
        resp.set_body("CGI script not found");
        return resp;
    }

    // Determine interpreter based on extension
    let interpreter = match script.extension().and_then(|s| s.to_str()) {
        Some("py") => find_python_interpreter(),
        Some("sh") => Some("sh".to_string()),
        _ => {
            let mut resp = HttpResponse::internal_error();
            resp.set_body("CGI Error: Unsupported script type");
            return resp;
        }
    };

    let interpreter = match interpreter {
        Some(i) => i,
        None => {
            let mut resp = HttpResponse::internal_error();
            resp.set_body("CGI Error: No interpreter found");
            return resp;
        }
    };

    // Build CGI environment variables
    let mut env: HashMap<String, String> = HashMap::new();
    env.insert("REQUEST_METHOD".to_string(), request.method.clone());
    env.insert("SCRIPT_NAME".to_string(), script_path.to_string());
    env.insert("PATH_INFO".to_string(), path_info.to_string());
    env.insert("CONTENT_LENGTH".to_string(), request.body.len().to_string());
    env.insert("SERVER_PROTOCOL".to_string(), "HTTP/1.1".to_string());
    env.insert("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string());
    env.insert("QUERY_STRING".to_string(), request.query.clone());
    
    if let Some(ct) = request.headers.get("Content-Type") {
        env.insert("CONTENT_TYPE".to_string(), ct.clone());
    }
    
    // Pass HTTP headers as environment variables
    for (k, v) in &request.headers {
        let env_key = format!("HTTP_{}", k.to_uppercase().replace("-", "_"));
        env.insert(env_key, v.clone());
    }

    // Set working directory to script's directory
    let working_dir = script.parent().map(|p| p.to_path_buf());
    
    // Get just the script filename for when we change working directory
    let script_filename = script.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(script_path);

    // Spawn CGI process
    let mut cmd = Command::new(&interpreter);
    
    // If we're setting a working directory, use just the filename
    // Otherwise use the full path
    if let Some(ref dir) = working_dir {
        if dir.exists() && !dir.as_os_str().is_empty() {
            cmd.arg(script_filename);
            cmd.current_dir(dir);
        } else {
            cmd.arg(script_path);
        }
    } else {
        cmd.arg(script_path);
    }
    
    cmd.envs(&env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let mut resp = HttpResponse::internal_error();
            resp.set_body(&format!("CGI spawn error: {}", e));
            return resp;
        }
    };

    // Get handles
    let mut stdin_handle = child.stdin.take();
    let mut stdout_handle = child.stdout.take().unwrap();
    let mut stderr_handle = child.stderr.take().unwrap();

    // Set non-blocking mode on stdout/stderr
    set_fd_nonblocking(stdout_handle.as_raw_fd());
    set_fd_nonblocking(stderr_handle.as_raw_fd());

    // Create epoll instance for CGI I/O
    let epoll_fd = unsafe { libc::epoll_create1(0) };
    if epoll_fd < 0 {
        let _ = child.kill();
        let mut resp = HttpResponse::internal_error();
        resp.set_body("CGI Error: epoll_create failed");
        return resp;
    }

    let stdout_fd = stdout_handle.as_raw_fd();
    let stderr_fd = stderr_handle.as_raw_fd();

    // Register stdout and stderr with epoll
    register_fd_epoll(epoll_fd, stdout_fd);
    register_fd_epoll(epoll_fd, stderr_fd);

    // Write request body to stdin (if any)
    if !request.body.is_empty() {
        if let Some(ref mut stdin) = stdin_handle {
            let _ = stdin.write_all(&request.body);
        }
    }
    // Drop stdin to send EOF to CGI
    drop(stdin_handle);

    // Read output using epoll (NON-BLOCKING)
    let timeout = Duration::from_secs(5);
    let start = Instant::now();
    
    let mut stdout_buf: Vec<u8> = Vec::with_capacity(8192);
    let mut stderr_buf: Vec<u8> = Vec::new();
    let mut stdout_done = false;
    let mut stderr_done = false;

    while !stdout_done || !stderr_done {
        // Check timeout
        if start.elapsed() > timeout {
            let _ = child.kill();
            unsafe { libc::close(epoll_fd) };
            let mut resp = HttpResponse::new(504, "Gateway Timeout");
            resp.set_body("CGI script timed out");
            return resp;
        }

        // Wait for events with short timeout (50ms)
        let mut events: [libc::epoll_event; 4] = [libc::epoll_event { events: 0, u64: 0 }; 4];
        let nfds = unsafe {
            libc::epoll_wait(epoll_fd, events.as_mut_ptr(), 4, 50)
        };

        if nfds < 0 {
            // epoll error, but might be EINTR
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::EINTR) {
                break;
            }
            continue;
        }

        // Process events
        for i in 0..nfds as usize {
            let ev = &events[i];
            let fd = ev.u64 as RawFd;

            if fd == stdout_fd && !stdout_done {
                match read_nonblocking(&mut stdout_handle, &mut stdout_buf) {
                    ReadResult::Data => {}
                    ReadResult::Eof => stdout_done = true,
                    ReadResult::WouldBlock => {}
                    ReadResult::Error => stdout_done = true,
                }
            }

            if fd == stderr_fd && !stderr_done {
                match read_nonblocking(&mut stderr_handle, &mut stderr_buf) {
                    ReadResult::Data => {}
                    ReadResult::Eof => stderr_done = true,
                    ReadResult::WouldBlock => {}
                    ReadResult::Error => stderr_done = true,
                }
            }
        }

        // Also check if child has exited
        match child.try_wait() {
            Ok(Some(_)) => {
                // Child exited, drain remaining output
                drain_remaining(&mut stdout_handle, &mut stdout_buf);
                drain_remaining(&mut stderr_handle, &mut stderr_buf);
                break;
            }
            Ok(None) => {} // Still running
            Err(_) => break,
        }
    }

    // Cleanup epoll
    unsafe { libc::close(epoll_fd) };

    // Wait for child to fully exit
    let _ = child.wait();

    // Build response
    if stdout_buf.is_empty() {
        let err_msg = String::from_utf8_lossy(&stderr_buf);
        let mut resp = HttpResponse::internal_error();
        if err_msg.trim().is_empty() {
            resp.set_body("CGI produced no output");
        } else {
            resp.set_body(&format!("CGI error: {}", err_msg));
        }
        return resp;
    }

    // Parse CGI output (headers + body)
    let (headers_part, body_part) = extract_cgi_headers_body(&stdout_buf);

    let mut resp = HttpResponse::ok();
    
    // Parse headers from CGI output
    for line in headers_part.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            
            // Handle Status header specially
            if key.eq_ignore_ascii_case("Status") {
                if let Some((code_str, text)) = val.split_once(' ') {
                    if let Ok(code) = code_str.parse::<u16>() {
                        resp.status_code = code;
                        resp.status_text = text.to_string();
                    }
                }
            } else {
                resp.set_header(key, val);
            }
        }
    }

    // Set default Content-Type if not provided
    if !resp.headers.contains_key("Content-Type") {
        resp.set_header("Content-Type", "text/plain; charset=utf-8");
    }

    resp.set_body_bytes(body_part.to_vec());
    resp
}

// Helper: Find Python interpreter
fn find_python_interpreter() -> Option<String> {
    for prog in &["python3", "python"] {
        if std::process::Command::new(prog)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
        {
            return Some(prog.to_string());
        }
    }
    None
}

// Helper: Set fd to non-blocking
fn set_fd_nonblocking(fd: RawFd) {
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags >= 0 {
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
    }
}

// Helper: Register fd with epoll
fn register_fd_epoll(epoll_fd: RawFd, fd: RawFd) {
    let mut ev = libc::epoll_event {
        events: (libc::EPOLLIN | libc::EPOLLET) as u32,
        u64: fd as u64,
    };
    unsafe {
        libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut ev);
    }
}

// Helper: Read result enum
enum ReadResult {
    Data,
    Eof,
    WouldBlock,
    Error,
}

// Helper: Non-blocking read
fn read_nonblocking<R: Read>(reader: &mut R, buf: &mut Vec<u8>) -> ReadResult {
    let mut temp = [0u8; 4096];
    match reader.read(&mut temp) {
        Ok(0) => ReadResult::Eof,
        Ok(n) => {
            buf.extend_from_slice(&temp[..n]);
            ReadResult::Data
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => ReadResult::WouldBlock,
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => ReadResult::WouldBlock,
        Err(_) => ReadResult::Error,
    }
}

// Helper: Drain remaining data from reader
fn drain_remaining<R: Read>(reader: &mut R, buf: &mut Vec<u8>) {
    let mut temp = [0u8; 4096];
    loop {
        match reader.read(&mut temp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&temp[..n]),
            Err(_) => break,
        }
    }
}

// Helper: Extract headers and body from CGI output
fn extract_cgi_headers_body(raw: &[u8]) -> (String, &[u8]) {
    // Look for \r\n\r\n or \n\n
    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
        let head = String::from_utf8_lossy(&raw[..pos]).to_string();
        let body = &raw[pos + 4..];
        return (head, body);
    }
    if let Some(pos) = raw.windows(2).position(|w| w == b"\n\n") {
        let head = String::from_utf8_lossy(&raw[..pos]).to_string();
        let body = &raw[pos + 2..];
        return (head, body);
    }
    // No headers found, treat all as body
    (String::new(), raw)
}