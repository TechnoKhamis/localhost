
## 1. Multi-Port Binding ✅

```bash
cargo run
# Expected: [listen] bound 127.0.0.1:8080, 8081, 8082
```

**Test all ports:**
```bash
curl http://127.0.0.1:8080/
curl http://127.0.0.1:8081/
curl http://127.0.0.1:8082/
# All return: 200 OK
```

---

## 2. Virtual Hosts ✅

```bash
# Setup
mkdir www_test www_demo
echo "<h1>Test.com</h1>" > www_test/index.html
echo "<h1>Demo.com</h1>" > www_demo/index.html

# Test
curl --resolve test.com:8080:127.0.0.1 http://test.com:8080/
# Returns: <h1>Test.com</h1>

curl --resolve demo.com:8080:127.0.0.1 http://demo.com:8080/
# Returns: <h1>Demo.com</h1>

curl --resolve broken.com:8080:127.0.0.1 http://broken.com:8080/
# Returns: 404 (directory doesn't exist)
```

---

## 3. Error Pages ✅

```bash
curl -i http://127.0.0.1:8080/notfound    # 404
curl -i -X PATCH http://127.0.0.1:8080/   # 405
curl -i --http1.0 http://127.0.0.1:8080/  # 400
```

---

## 4. File Upload → Download → Delete Flow ✅

```bash
# Create test file
dd if=/dev/urandom of=/tmp/test.bin bs=1M count=2

# Upload
curl -X POST --data-binary @/tmp/test.bin -H "X-Filename: test.bin" http://127.0.0.1:8080/upload
# Returns: 200 OK

# Verify file exists
ls uploads/test.bin

# Download
curl -o /tmp/test-down.bin http://127.0.0.1:8080/files/test.bin

# Check integrity
sha256sum /tmp/test.bin /tmp/test-down.bin
# Must match!

# Delete
curl -i -X DELETE http://127.0.0.1:8080/upload/test.bin
# Returns: 200 OK

# Verify deleted
curl -i http://127.0.0.1:8080/files/test.bin #GMM
# Returns: 404
```

---

## 5. Upload Size Limit (10MB) ✅

```bash
# Small file - pass
echo "small" > small.txt
curl -X POST --data-binary @small.txt -H "X-Filename: small.txt" http://127.0.0.1:8080/upload
# Returns: 200 OK

# Large file - fail
dd if=/dev/zero of=huge.bin bs=1M count=12
curl -i -X POST --data-binary @huge.bin -H "X-Filename: huge.bin" http://127.0.0.1:8080/upload
# Returns: 413 Payload Too Large
```

---

## 6. Directory Autoindex ✅

```bash
curl http://127.0.0.1:8080/public/
# Returns: <h1>Index of /public</h1><ul>...
```

---

## 7. Redirect ✅

```bash
curl -i http://127.0.0.1:8080/docs
# HTTP/1.1 302 Found
# Location: /
```

---

## 8. CGI (Chunked + Unchunked) ✅

**Setup:**
```bash
cat > cgi-bin/test.py << 'EOF'
#!/usr/bin/env python3
import sys, os
print("Content-Type: text/plain")
print()
print("CGI OK")
print("Method:", os.environ.get("REQUEST_METHOD"))
body = sys.stdin.read()
if body: print("Body:", body)
EOF
chmod +x cgi-bin/test.py
```

**Test unchunked:**
```bash
curl -X POST -d "data=test" http://127.0.0.1:8080/cgi/test.py
# Returns: CGI OK, Method: POST, Body: data=test
```

**Test chunked:**
```bash
printf 'chunked\n' | curl -X POST -H "Transfer-Encoding: chunked" --data-binary @- http://127.0.0.1:8080/cgi/test.py
# Returns: CGI OK, Method: POST, Body: chunked
```

---

## 9. Session Cookies s

```bash
curl -i http://127.0.0.1:8080/ | grep Set-Cookie
# Returns: Set-Cookie: SID=...

```

---

## 10. Method Restrictions ✅

```bash
curl -X POST http://127.0.0.1:8080/upload     # 200 OK
curl -X DELETE http://127.0.0.1:8080/upload/file.txt  # 200 OK
curl -i -X GET http://127.0.0.1:8080/upload   # 405 Method Not Allowed
```

---

## 11. Stress Test (Siege) ✅

```bash
siege -b -t30s http://127.0.0.1:8080/
# Availability: ≥99.5% required
```

---

## 12. Memory & Connections ✅

```bash
# Monitor memory during siege
watch -n1 "ps -o rss -p $(pidof localhost)"
# RSS should stay stable

```

---

**All tests complete! Server ready for evaluation.** ✅