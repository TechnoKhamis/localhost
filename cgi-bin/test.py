#!/usr/bin/env python3
import sys
import os

print("Content-Type: text/plain")
print()
print("CGI OK")
print(f"Method: {os.environ.get('REQUEST_METHOD', 'N/A')}")
print(f"Query: {os.environ.get('QUERY_STRING', 'N/A')}")

# Read body
body = sys.stdin.read()
if body:
    print(f"Body: {body}")
