use crate::http::{HttpRequest, HttpResponse};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

/// Run a CGI script with timeout protection
pub fn run_cgi(script_path: &str, path_info: &str, request: &HttpRequest) -> HttpResponse {
    let script = Path::new(script_path);
    
    // Determine interpreter
    let interpreter = match script.extension().and_then(|s| s.to_str()) {
        Some("py") => "python3",
        _ => {
            let mut resp = HttpResponse::internal_error();
            resp.set_body("CGI Error: Unsupported script type");
            return resp;
        }
    };
    
    // Build environment
    let mut env = HashMap::new();
    env.insert("REQUEST_METHOD".to_string(), request.method.clone());
    env.insert("SCRIPT_NAME".to_string(), script_path.to_string());
    env.insert("PATH_INFO".to_string(), path_info.to_string());
    env.insert("CONTENT_LENGTH".to_string(), request.body.len().to_string());
    env.insert("SERVER_PROTOCOL".to_string(), "HTTP/1.1".to_string());
    env.insert("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string());
    
    if let Some(ct) = request.headers.get("Content-Type") {
        env.insert("CONTENT_TYPE".to_string(), ct.clone());
    }
    
    // Query string
    env.insert("QUERY_STRING".to_string(), request.query.clone());
    
    // HTTP headers
    for (k, v) in &request.headers {
        env.insert(format!("HTTP_{}", k.to_uppercase().replace("-", "_")), v.clone());
    }
    
    // Spawn process
    let mut child = match Command::new(interpreter)
        .arg(script_path)
        .envs(&env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let mut resp = HttpResponse::internal_error();
            resp.set_body(&format!("CGI spawn error: {}", e));
            return resp;
        }
    };
    
    // Write request body
    if !request.body.is_empty() {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(&request.body);
        }
    }
    
    // Wait with timeout
    let timeout = Duration::from_secs(5);
    let start = Instant::now();
    
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let mut resp = HttpResponse::new(504, "Gateway Timeout");
                    resp.set_body("CGI script timed out");
                    return resp;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                let mut resp = HttpResponse::internal_error();
                resp.set_body(&format!("CGI wait error: {}", e));
                return resp;
            }
        }
    }
    
    // Read output
    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            let mut resp = HttpResponse::internal_error();
            resp.set_body(&format!("CGI read error: {}", e));
            return resp;
        }
    };
    
    // Parse output
    let (headers_part, body_part) = extract_headers_body(&output.stdout);
    
    let mut resp = HttpResponse::ok();
    for line in headers_part.lines() {
        if let Some((k, v)) = line.split_once(':') {
            resp.set_header(k.trim(), v.trim());
        }
    }
    
    if !resp.headers.contains_key("Content-Type") {
        resp.set_header("Content-Type", "text/plain; charset=utf-8");
    }
    
    resp.set_body_bytes(body_part.to_vec());
    resp
}

fn extract_headers_body(raw: &[u8]) -> (String, &[u8]) {
    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
        (String::from_utf8_lossy(&raw[..pos]).to_string(), &raw[pos + 4..])
    } else if let Some(pos) = raw.windows(2).position(|w| w == b"\n\n") {
        (String::from_utf8_lossy(&raw[..pos]).to_string(), &raw[pos + 2..])
    } else {
        (String::new(), raw)
    }
}
