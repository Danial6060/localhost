use std::collections::HashMap;

pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status_code: u16) -> Self {
        let status_text = Self::status_text(status_code);
        
        let mut headers = HashMap::new();
        headers.insert("Server".to_string(), "Webserv/1.0".to_string());
        headers.insert("Connection".to_string(), "keep-alive".to_string());

        HttpResponse {
            status_code,
            status_text,
            headers,
            body: Vec::new(),
        }
    }

    pub fn status_text(code: u16) -> String {
        match code {
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            301 => "Moved Permanently",
            302 => "Found",
            304 => "Not Modified",
            400 => "Bad Request",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            413 => "Payload Too Large",
            500 => "Internal Server Error",
            501 => "Not Implemented",
            _ => "Unknown",
        }.to_string()
    }

    pub fn set_body(&mut self, body: Vec<u8>) {
        self.headers.insert("Content-Length".to_string(), body.len().to_string());
        self.body = body;
    }

    pub fn set_body_str(&mut self, body: &str) {
        self.set_body(body.as_bytes().to_vec());
    }

    pub fn add_header(&mut self, key: String, value: String) {
        self.headers.insert(key, value);
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {} {}\r\n",
            self.status_code,
            self.status_text
        );

        for (key, value) in &self.headers {
            response.push_str(&format!("{}: {}\r\n", key, value));
        }

        response.push_str("\r\n");

        let mut bytes = response.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }

pub fn error_page(code: u16, custom_page: Option<&str>) -> Self {
        let mut response = HttpResponse::new(code);
        
        if let Some(page_path) = custom_page {
            if let Ok(content) = std::fs::read(page_path) {
                response.add_header("Content-Type".to_string(), "text/html".to_string());
                response.set_body(content);
                return response;
            }
        }

        // Default error page
        let body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>{} {}</title>
    <style>
        body {{ font-family: Arial, sans-serif; text-align: center; padding: 50px; }}
        h1 {{ color: #333; }}
        p {{ color: #666; }}
    </style>
</head>
<body>
    <h1>{} {}</h1>
    <p>The server encountered an error processing your request.</p>
    <hr>
    <small>Webserv/1.0</small>
</body>
</html>"#,
            code,
            Self::status_text(code),
            code,
            Self::status_text(code)
        );

        response.add_header("Content-Type".to_string(), "text/html".to_string());
        response.set_body_str(&body);
        response
    }

    pub fn directory_listing(path: &str, uri: &str, entries: Vec<String>) -> Self {
        let mut response = HttpResponse::new(200);
        
        let mut body = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <title>Index of {}</title>
    <style>
        body {{ font-family: monospace; padding: 20px; }}
        a {{ display: block; padding: 5px; text-decoration: none; color: #0066cc; }}
        a:hover {{ background: #f0f0f0; }}
    </style>
</head>
<body>
    <h1>Index of {}</h1>
    <hr>
"#,
            uri, uri
        );

        if uri != "/" {
            body.push_str(r#"<a href="../">../</a>"#);
        }

        for entry in entries {
            let display_name = if std::fs::metadata(format!("{}/{}", path, entry))
                .map(|m| m.is_dir())
                .unwrap_or(false)
            {
                format!("{}/", entry)
            } else {
                entry.clone()
            };

            body.push_str(&format!(
                r#"<a href="{}{}">{}</a>"#,
                if uri.ends_with('/') { "" } else { "/" },
                entry,
                display_name
            ));
        }

        body.push_str("</body></html>");

        response.add_header("Content-Type".to_string(), "text/html".to_string());
        response.set_body_str(&body);
        response
    }
}