#!/usr/bin/env python3
import sys
import os

print("Content-Type: text/html")
print("Status: 200 OK")
print()

print("<!DOCTYPE html>")
print("<html>")
print("<head>")
print("<title>CGI Test Script</title>")
print("<style>")
print("body { font-family: monospace; padding: 20px; background: #f9f9f9; }")
print("h1 { color: #667eea; }")
print("table { border-collapse: collapse; width: 100%; margin: 20px 0; }")
print("th, td { border: 1px solid #ddd; padding: 12px; text-align: left; }")
print("th { background: #667eea; color: white; }")
print("tr:nth-child(even) { background: #f2f2f2; }")
print(".success { color: #4CAF50; font-weight: bold; }")
print("</style>")
print("</head>")
print("<body>")
print("<h1 class='success'>✅ CGI Script Executed Successfully!</h1>")

print("<h2>Environment Variables:</h2>")
print("<table>")
print("<tr><th>Variable</th><th>Value</th></tr>")

env_vars = [
    'REQUEST_METHOD',
    'QUERY_STRING',
    'CONTENT_TYPE',
    'CONTENT_LENGTH',
    'SCRIPT_NAME',
    'SCRIPT_FILENAME',
    'PATH_INFO',
    'SERVER_NAME',
    'SERVER_PORT',
    'SERVER_PROTOCOL',
    'GATEWAY_INTERFACE',
    'REMOTE_ADDR',
]

for var in env_vars:
    value = os.environ.get(var, 'N/A')
    print(f"<tr><td><strong>{var}</strong></td><td>{value}</td></tr>")

print("</table>")

query = os.environ.get('QUERY_STRING', '')
if query:
    print("<h2>Query Parameters:</h2>")
    print("<table>")
    print("<tr><th>Key</th><th>Value</th></tr>")
    for param in query.split('&'):
        if '=' in param:
            key, value = param.split('=', 1)
            print(f"<tr><td><strong>{key}</strong></td><td>{value}</td></tr>")
    print("</table>")

content_length = os.environ.get('CONTENT_LENGTH', '0')
if content_length and int(content_length) > 0:
    post_data = sys.stdin.read(int(content_length))
    print("<h2>POST Data:</h2>")
    print(f"<pre style='background: #f0f0f0; padding: 15px; border-radius: 5px;'>{post_data}</pre>")

print("</body>")
print("</html>")
