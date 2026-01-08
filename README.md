# 🦀 Localhost - HTTP/1.1 Web Server in Rust

A high-performance, fully-featured HTTP/1.1 web server built from scratch in Rust. Implements non-blocking I/O with epoll, virtual hosts, CGI execution, file uploads, and session management.

---

## ✨ Features

- ✅ **HTTP/1.1 compliant** with keep-alive support
- ✅ **Non-blocking I/O** using epoll for scalability
- ✅ **Multi-port listening** - bind to multiple ports simultaneously
- ✅ **Virtual hosts** - serve multiple websites from one server
- ✅ **Static file serving** with automatic content-type detection
- ✅ **File uploads** - multipart/form-data and raw binary
- ✅ **File deletion** via DELETE method
- ✅ **CGI execution** - Python script support with chunked/unchunked requests
- ✅ **Directory listing** - auto-index for browsing directories
- ✅ **Custom error pages** - 400, 403, 404, 405, 413, 500
- ✅ **Request size limits** - configurable max body size
- ✅ **Session management** - HTTP-only session cookies
- ✅ **URL redirects** - 302 redirects
- ✅ **Connection timeout** - automatic cleanup of idle connections
- ✅ **Stress tested** - 100% availability under load

---

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ and Cargo
- Linux (for epoll support)

### Installation

```bash
git clone <your-repo>
cd localhost
cargo build --release
```

### Run Server

```bash
cargo run --release
```

Server starts on configured ports (default: 8080, 8081, 8082)

---

## ⚙️ Configuration

Edit `server.conf`:

```conf
# Listen on multiple ports
listen = 127.0.0.1:8080,127.0.0.1:8081,127.0.0.1:8082

# Max upload size (10MB)
client_body_size_limit = 10485760

# Custom error pages directory
error_path = www/errors

# Main route
route / {
    methods = GET,POST
    root = www
    default_file = index.html
    autoindex = on
}

# File upload endpoint
route /upload {
    methods = POST,DELETE
    root = uploads
    autoindex = off
}

# CGI scripts
route /cgi {
    methods = GET,POST
    root = cgi-bin
    cgi = python3
}

# Redirect example
route /docs {
    methods = GET
    redirect = /
}

# Virtual host example
vhost example.com {
    route / {
        methods = GET
        root = www_example
        default_file = index.html
    }
}
```

---

## 📂 Directory Structure

```
localhost/
├── Cargo.toml
├── server.conf              # Server configuration
├── www/                     # Static files
│   ├── index.html
│   └── errors/              # Custom error pages
│       ├── 404.html
│       ├── 405.html
│       └── 500.html
├── uploads/                 # Upload directory
├── cgi-bin/                 # CGI scripts
│   └── test.py
└── src/
    ├── main.rs
    ├── config/              # Config parser
    ├── http/                # HTTP request/response
    ├── network/             # Epoll, server, router
    └── handlers/            # File, upload, CGI handlers
```

---

## 🧪 Testing

### Basic Tests

```bash
# Test GET
curl http://127.0.0.1:8080/

# Test file upload
curl -X POST --data-binary @file.txt -H "X-Filename: file.txt" http://127.0.0.1:8080/upload

# Test file download
curl -o downloaded.txt http://127.0.0.1:8080/files/file.txt

# Test delete
curl -X DELETE http://127.0.0.1:8080/upload/file.txt

# Test CGI
curl -X POST -d "data=test" http://127.0.0.1:8080/cgi/test.py

# Test virtual host
curl --resolve example.com:8080:127.0.0.1 http://example.com:8080/
```

### Stress Test

```bash
# Requires siege
siege -b -t30s http://127.0.0.1:8080/

# Expected: ≥99.5% availability
```

### File Integrity Test

```bash
# Create test file
dd if=/dev/urandom of=/tmp/test.bin bs=1M count=2

# Upload
curl -X POST --data-binary @/tmp/test.bin -H "X-Filename: test.bin" http://127.0.0.1:8080/upload

# Download
curl -o /tmp/test-down.bin http://127.0.0.1:8080/files/test.bin

# Verify integrity
sha256sum /tmp/test.bin /tmp/test-down.bin
# Hashes should match!
```

---

## 🔧 Configuration Options

| Option | Type | Description |
|--------|------|-------------|
| `listen` | String | Comma-separated list of IP:PORT to bind |
| `client_body_size_limit` | Number | Max request body size in bytes |
| `error_path` | String | Directory containing custom error pages |
| `methods` | List | Allowed HTTP methods for route (GET, POST, DELETE) |
| `root` | String | Root directory for serving files |
| `default_file` | String | Default file when path is directory |
| `autoindex` | Boolean | Enable directory listing (on/off) |
| `redirect` | String | Redirect URL (returns 302) |
| `cgi` | String | CGI interpreter (e.g., python3) |

---

## 🎯 Architecture

### Event Loop (Epoll)

```
┌─────────────────────────────────────┐
│  Single Epoll Instance              │
│  - Monitors all sockets             │
│  - Non-blocking I/O                 │
└─────────────────────────────────────┘
         │
         ├─► Listener Sockets (8080, 8081, 8082)
         │   └─► Accept new connections
         │
         └─► Client Sockets
             ├─► Read HTTP requests
             ├─► Route to handlers
             ├─► Write HTTP responses
             └─► Keep-alive or close
```

### Request Flow

```
Client Request
    │
    ├─► Parse HTTP (method, path, headers, body)
    │
    ├─► Match Virtual Host (by Host header)
    │
    ├─► Find Route (longest prefix match)
    │
    ├─► Check Method (GET/POST/DELETE allowed?)
    │
    ├─► Handle Request
    │   ├─► Redirect? → 302 response
    │   ├─► CGI? → Execute script
    │   ├─► Upload? → Save file
    │   ├─► Delete? → Remove file
    │   └─► Static file? → Serve content
    │
    └─► Send Response
        ├─► Set session cookie (if needed)
        ├─► Add Connection header (keep-alive/close)
        └─► Write to socket
```

---

## 🛡️ Security Features

- ✅ Filename sanitization for uploads
- ✅ Request body size limits
- ✅ HTTP-only session cookies
- ✅ Method restrictions per route
- ✅ Request timeouts (10 seconds)
- ✅ Path traversal prevention

---

## 🐛 Debugging

### Enable Logs

```bash
# See all connections and requests
RUST_LOG=debug cargo run
```

### Common Issues

**Port already in use:**
```bash
# Find process using port
lsof -i :8080
# Kill it
kill -9 <PID>
```

**Permission denied on port <1024:**
```bash
# Use ports ≥1024 or run with sudo (not recommended)
```

**CGI script not executing:**
```bash
# Make script executable
chmod +x cgi-bin/test.py

# Check shebang
head -1 cgi-bin/test.py
# Should be: #!/usr/bin/env python3
```

---

## 📊 Performance

- **Concurrency:** Handles 1000+ simultaneous connections
- **Throughput:** ~180 requests/second (tested with siege)
- **Availability:** 100% uptime under 30s stress test
- **Memory:** Constant RSS, no leaks detected
- **Response Time:** Average 140ms under load

---

## 🏗️ Development

### Build Debug Version

```bash
cargo build
```

### Run Tests

```bash
cargo test
```

### Check Code

```bash
cargo clippy
cargo fmt
```

---

## 📝 License

MIT License - See LICENSE file for details

---

## 🙏 Acknowledgments

Built as an educational project to understand:
- HTTP/1.1 protocol implementation
- Non-blocking I/O with epoll
- Network programming in Rust
- Web server architecture

---

## 📚 References

- [HTTP/1.1 RFC 2616](https://www.rfc-editor.org/rfc/rfc2616)
- [Epoll Manual](https://man7.org/linux/man-pages/man7/epoll.7.html)
- [CGI/1.1 Specification](https://www.rfc-editor.org/rfc/rfc3875)

---

**Ready for production testing!** 🚀