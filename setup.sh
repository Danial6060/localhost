#!/bin/bash

# Setup script for Webserv HTTP Server

echo "========================================="
echo "  Webserv Setup Script"
echo "========================================="
echo

# Create directory structure
echo "Creating directory structure..."
mkdir -p www/uploads
mkdir -p www/static
mkdir -p cgi-bin
mkdir -p errors

# Create sample HTML files
echo "Creating sample HTML files..."

# Main index.html
cat > www/index.html << 'EOF'
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Webserv Test Page</title>
    <style>
        body {
            font-family: Arial, sans-serif;
            max-width: 900px;
            margin: 50px auto;
            padding: 20px;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        }
        .container {
            background: white;
            padding: 40px;
            border-radius: 10px;
            box-shadow: 0 10px 30px rgba(0,0,0,0.3);
        }
        h1 { color: #667eea; margin-bottom: 10px; }
        h2 { color: #764ba2; border-bottom: 2px solid #667eea; padding-bottom: 10px; }
        .test-section {
            margin: 20px 0;
            padding: 20px;
            border-left: 4px solid #667eea;
            background: #f9f9f9;
        }
        button {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            border: none;
            padding: 12px 24px;
            cursor: pointer;
            border-radius: 5px;
            margin: 5px;
            font-size: 14px;
            transition: transform 0.2s;
        }
        button:hover { transform: scale(1.05); }
        #output {
            margin-top: 20px;
            padding: 15px;
            background: #f0f0f0;
            border-radius: 5px;
            min-height: 60px;
            font-family: monospace;
            white-space: pre-wrap;
        }
        .success { color: #4CAF50; }
        .error { color: #f44336; }
        input[type="file"] {
            margin: 10px 0;
            padding: 5px;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 Webserv HTTP/1.1 Server</h1>
        <p>Welcome! This is a fully functional HTTP/1.1 server written in Rust.</p>

        <div class="test-section">
            <h2>🧪 Test Suite</h2>
            
            <h3>GET Request</h3>
            <button onclick="testGet()">Test GET</button>
            <button onclick="testGet404()">Test 404 Error</button>
            
            <h3>POST Request</h3>
            <button onclick="testPost()">Send POST Data</button>
            
            <h3>DELETE Request</h3>
            <button onclick="testDelete()">Test DELETE</button>
            
            <h3>Session & Cookies</h3>
            <button onclick="testCookie()">Check Cookies</button>
            <button onclick="clearCookies()">Clear Cookies</button>
            
            <h3>File Upload</h3>
            <input type="file" id="fileInput">
            <button onclick="uploadFile()">Upload File</button>
            
            <h3>CGI Script</h3>
            <button onclick="testCGI()">Execute CGI Script</button>
            <button onclick="testCGIPost()">CGI with POST Data</button>
            
            <h3>Other Tests</h3>
            <button onclick="testRedirect()">Test Redirect</button>
            <button onclick="testListing()">Test Directory Listing</button>
        </div>

        <div id="output">Output will appear here...</div>
    </div>

    <script>
        function log(message, type = 'info') {
            const output = document.getElementById('output');
            const className = type === 'error' ? 'error' : type === 'success' ? 'success' : '';
            output.innerHTML = `<span class="${className}">${message}</span>`;
        }

        function testGet() {
            log('Testing GET request...');
            fetch('/static/test.txt')
                .then(response => {
                    if (!response.ok) throw new Error('Status: ' + response.status);
                    return response.text();
                })
                .then(data => log('✅ GET Success!\\n' + data, 'success'))
                .catch(err => log('❌ GET Failed: ' + err, 'error'));
        }

        function testGet404() {
            log('Testing 404 error...');
            fetch('/nonexistent-file')
                .then(response => {
                    if (response.status === 404) {
                        return response.text().then(text => 
                            log('✅ 404 Error handled correctly!\\nStatus: ' + response.status, 'success')
                        );
                    }
                    throw new Error('Expected 404, got: ' + response.status);
                })
                .catch(err => log('Test result: ' + err, 'error'));
        }

        function testPost() {
            log('Testing POST request...');
            fetch('/', {
                method: 'POST',
                headers: {'Content-Type': 'application/json'},
                body: JSON.stringify({message: 'Hello from browser!', timestamp: Date.now()})
            })
            .then(response => response.text())
            .then(data => log('✅ POST Success!\\n' + data, 'success'))
            .catch(err => log('❌ POST Failed: ' + err, 'error'));
        }

        function testDelete() {
            log('Testing DELETE request...');
            fetch('/uploads/test.txt', {method: 'DELETE'})
            .then(response => {
                if (response.status === 204 || response.status === 404) {
                    log('✅ DELETE executed!\\nStatus: ' + response.status, 'success');
                } else {
                    throw new Error('Unexpected status: ' + response.status);
                }
            })
            .catch(err => log('❌ DELETE test: ' + err, 'error'));
        }

        function testCookie() {
            log('Checking cookies and session...');
            const cookies = document.cookie;
            if (cookies) {
                log('✅ Cookies found:\\n' + cookies, 'success');
            } else {
                log('No cookies set. The server should set a session cookie on first visit.', 'error');
            }
        }

        function clearCookies() {
            document.cookie.split(";").forEach(c => {
                document.cookie = c.replace(/^ +/, "").replace(/=.*/, "=;expires=" + new Date().toUTCString() + ";path=/");
            });
            log('✅ Cookies cleared!', 'success');
        }

        function uploadFile() {
            const fileInput = document.getElementById('fileInput');
            if (!fileInput.files[0]) {
                log('❌ Please select a file first!', 'error');
                return;
            }

            log('Uploading file: ' + fileInput.files[0].name + '...');
            const formData = new FormData();
            formData.append('file', fileInput.files[0]);

            fetch('/uploads', {
                method: 'POST',
                body: formData
            })
            .then(response => response.text())
            .then(data => log('✅ Upload Success!\\n' + data, 'success'))
            .catch(err => log('❌ Upload Failed: ' + err, 'error'));
        }

        function testCGI() {
            log('Testing CGI script...');
            fetch('/cgi-bin/test.py?name=Browser&value=42')
                .then(response => response.text())
                .then(data => {
                    const preview = data.substring(0, 500);
                    log('✅ CGI Success!\\n' + preview + '\\n...', 'success');
                })
                .catch(err => log('❌ CGI Failed: ' + err, 'error'));
        }

        function testCGIPost() {
            log('Testing CGI with POST data...');
            fetch('/cgi-bin/test.py', {
                method: 'POST',
                headers: {'Content-Type': 'application/x-www-form-urlencoded'},
                body: 'field1=value1&field2=value2'
            })
            .then(response => response.text())
            .then(data => {
                const preview = data.substring(0, 500);
                log('✅ CGI POST Success!\\n' + preview + '\\n...', 'success');
            })
            .catch(err => log('❌ CGI POST Failed: ' + err, 'error'));
        }

        function testRedirect() {
            log('Testing redirect...');
            fetch('/redirect', {redirect: 'manual'})
                .then(response => {
                    if (response.type === 'opaqueredirect' || response.status === 301 || response.status === 302) {
                        log('✅ Redirect working! Status: ' + response.status, 'success');
                    } else {
                        log('Status: ' + response.status, 'info');
                    }
                })
                .catch(err => log('Redirect result: ' + err, 'error'));
        }

        function testListing() {
            log('Testing directory listing...');
            fetch('/uploads/')
                .then(response => response.text())
                .then(data => {
                    if (data.includes('Index of')) {
                        log('✅ Directory listing works!', 'success');
                    } else {
                        log('Response received but no directory listing found', 'error');
                    }
                })
                .catch(err => log('❌ Listing Failed: ' + err, 'error'));
        }

        window.onload = function() {
            setTimeout(() => {
                if (document.cookie) {
                    console.log('Session cookie detected:', document.cookie);
                }
            }, 500);
        };
    </script>
</body>
</html>
EOF

# Create test file
echo "Hello from Webserv! This is a test file." > www/static/test.txt

# Create error pages
cat > errors/404.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>404 Not Found</title>
    <style>
        body { font-family: Arial; text-align: center; padding: 50px; background: #f5f5f5; }
        h1 { color: #e74c3c; font-size: 72px; margin: 0; }
        p { color: #555; font-size: 18px; }
        a { color: #3498db; text-decoration: none; }
    </style>
</head>
<body>
    <h1>404</h1>
    <p>Page not found</p>
    <a href="/">Go to homepage</a>
</body>
</html>
EOF

cat > errors/500.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>500 Internal Server Error</title>
    <style>
        body { font-family: Arial; text-align: center; padding: 50px; background: #f5f5f5; }
        h1 { color: #e74c3c; font-size: 72px; margin: 0; }
        p { color: #555; font-size: 18px; }
    </style>
</head>
<body>
    <h1>500</h1>
    <p>Internal Server Error</p>
</body>
</html>
EOF

# Create CGI test script
cat > cgi-bin/test.py << 'EOF'
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
EOF

chmod +x cgi-bin/test.py

echo "✅ Directory structure created"
echo "✅ Sample files created"
echo "✅ CGI script created and made executable"
echo

if [ ! -f "config.conf" ]; then
    echo "✅ Configuration file already created"
else
    echo "⚠️  config.conf already exists, skipping"
fi

echo
echo "========================================="
echo "  Setup Complete!"
echo "========================================="
echo
echo "Next steps:"
echo "  1. Build the server:"
echo "     cargo build --release"
echo
echo "  2. Run the server:"
echo "     ./target/release/webserv config.conf"
echo
echo "  3. Test in browser:"
echo "     http://127.0.0.1:8080/"
echo
echo "  4. Run test suite:"
echo "     ./tests.sh"
echo
echo "========================================="