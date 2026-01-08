use crate::http::{HttpRequest, HttpResponse};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::io::Write;
use std::path::Path;

/// Run a Python CGI script and return HttpResponse
pub fn run_cgi(script_path: &str, path_info: &str, request: &HttpRequest) -> HttpResponse {
    let script_path_buf = Path::new(script_path);

    // Only handle .py scripts for now
    let interpreter = match script_path_buf.extension().and_then(|s| s.to_str()) {
        Some("py") => if cfg!(windows) { "python" } else { "python3" },
        _ => {
            let mut resp = HttpResponse::internal_error();
            resp.set_body("CGI Error: Unsupported script type");
            return resp;
        }
    };

    // Build CGI environment
    let mut env = HashMap::new();
    env.insert("REQUEST_METHOD".to_string(), request.method.clone());
    env.insert("SCRIPT_NAME".to_string(), script_path.to_string());
    env.insert("PATH_INFO".to_string(), path_info.to_string());
    env.insert("CONTENT_LENGTH".to_string(), request.body.len().to_string());
    env.insert("SERVER_PROTOCOL".to_string(), "HTTP/1.1".to_string());
    env.insert("SCRIPT_FILENAME".to_string(), script_path.to_string());
    env.insert("GATEWAY_INTERFACE".to_string(), "CGI/1.1".to_string());
        

    if let Some(ct) = request.headers.get("Content-Type") {
        env.insert("CONTENT_TYPE".to_string(), ct.clone());
    }

    // Query string
    if let Some(pos) = request.path.find('?') {
        env.insert("QUERY_STRING".to_string(), request.path[pos + 1..].to_string());
    } else {
        env.insert("QUERY_STRING".to_string(), String::new());
    }

    // HTTP_* headers
    for (k, v) in &request.headers {
        env.insert(format!("HTTP_{}", k.to_uppercase().replace("-", "_")), v.clone());
    }

    // Spawn CGI process
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

    // Write request body if any
    if !request.body.is_empty() {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(&request.body);
        }
    }

    // Wait for output
    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            let mut resp = HttpResponse::internal_error();
            resp.set_body(&format!("CGI read error: {}", e));
            return resp;
        }
    };

    // If script fails
    if !output.status.success() {
        let mut resp = HttpResponse::internal_error();
        resp.set_body(&format!("CGI script error: {}", String::from_utf8_lossy(&output.stderr)));
        return resp;
    }

    // Split headers and body
    let (headers_part, body_part) = extract_headers_body(&output.stdout);

    // Build response
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

/// Split CGI output into headers and body
fn extract_headers_body(raw: &[u8]) -> (String, &[u8]) {
    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
        (String::from_utf8_lossy(&raw[..pos]).to_string(), &raw[pos + 4..])
    } else if let Some(pos) = raw.windows(2).position(|w| w == b"\n\n") {
        (String::from_utf8_lossy(&raw[..pos]).to_string(), &raw[pos + 2..])
    } else {
        (String::new(), raw)
    }
}
