use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub complete: bool,
}

impl HttpRequest {
    pub fn new() -> Self {
        HttpRequest {
            method: String::new(),
            uri: String::new(),
            version: String::new(),
            headers: HashMap::new(),
            body: Vec::new(),
            complete: false,
        }
    }
}

pub struct HttpParser {
    state: ParserState,
    buffer: Vec<u8>,
    headers_complete: bool,
    content_length: Option<usize>,
    is_chunked: bool,
    chunk_size: usize,
    chunk_state: ChunkState,
}

#[derive(PartialEq)]
enum ParserState {
    RequestLine,
    Headers,
    Body,
    Done,
}

#[derive(PartialEq)]
enum ChunkState {
    Size,
    Data,
    TrailingCRLF,
}

impl HttpParser {
    pub fn new() -> Self {
        HttpParser {
            state: ParserState::RequestLine,
            buffer: Vec::new(),
            headers_complete: false,
            content_length: None,
            is_chunked: false,
            chunk_size: 0,
            chunk_state: ChunkState::Size,
        }
    }

    pub fn parse(&mut self, data: &[u8], request: &mut HttpRequest) -> Result<(), String> {
        self.buffer.extend_from_slice(data);

        loop {
            match self.state {
                ParserState::RequestLine => {
                    if !self.parse_request_line(request)? {
                        return Ok(());
                    }
                    self.state = ParserState::Headers;
                }
                ParserState::Headers => {
                    if !self.parse_headers(request)? {
                        return Ok(());
                    }
                    self.headers_complete = true;
                    
                    // Check for Content-Length or Transfer-Encoding
                    if let Some(cl) = request.headers.get("content-length") {
                        self.content_length = cl.parse().ok();
                    }
                    
                    if let Some(te) = request.headers.get("transfer-encoding") {
                        if te.to_lowercase().contains("chunked") {
                            self.is_chunked = true;
                        }
                    }

                    if self.content_length.is_some() || self.is_chunked {
                        self.state = ParserState::Body;
                    } else {
                        self.state = ParserState::Done;
                        request.complete = true;
                        return Ok(());
                    }
                }
                ParserState::Body => {
                    if self.is_chunked {
                        if !self.parse_chunked_body(request)? {
                            return Ok(());
                        }
                    } else {
                        if !self.parse_body(request)? {
                            return Ok(());
                        }
                    }
                    self.state = ParserState::Done;
                    request.complete = true;
                    return Ok(());
                }
                ParserState::Done => {
                    request.complete = true;
                    return Ok(());
                }
            }
        }
    }

    fn parse_request_line(&mut self, request: &mut HttpRequest) -> Result<bool, String> {
        if let Some(pos) = self.find_crlf() {
            let line = String::from_utf8_lossy(&self.buffer[..pos]);
            let parts: Vec<&str> = line.split_whitespace().collect();

            if parts.len() != 3 {
                return Err("Invalid request line".to_string());
            }

            request.method = parts[0].to_uppercase();
            request.uri = parts[1].to_string();
            request.version = parts[2].to_string();

            self.buffer.drain(..pos + 2);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn parse_headers(&mut self, request: &mut HttpRequest) -> Result<bool, String> {
        loop {
            if let Some(pos) = self.find_crlf() {
                if pos == 0 {
                    // Empty line, headers complete
                    self.buffer.drain(..2);
                    return Ok(true);
                }

                let line = String::from_utf8_lossy(&self.buffer[..pos]);
                if let Some(colon_pos) = line.find(':') {
                    let key = line[..colon_pos].trim().to_lowercase();
                    let value = line[colon_pos + 1..].trim().to_string();
                    request.headers.insert(key, value);
                }

                self.buffer.drain(..pos + 2);
            } else {
                return Ok(false);
            }
        }
    }

    fn parse_body(&mut self, request: &mut HttpRequest) -> Result<bool, String> {
        if let Some(content_length) = self.content_length {
            if self.buffer.len() >= content_length {
                request.body.extend_from_slice(&self.buffer[..content_length]);
                self.buffer.drain(..content_length);
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn parse_chunked_body(&mut self, request: &mut HttpRequest) -> Result<bool, String> {
        loop {
            match self.chunk_state {
                ChunkState::Size => {
                    if let Some(pos) = self.find_crlf() {
                        let size_str = String::from_utf8_lossy(&self.buffer[..pos]);
                        self.chunk_size = usize::from_str_radix(
                            size_str.split(';').next().unwrap_or("0").trim(),
                            16
                        ).map_err(|_| "Invalid chunk size")?;

                        self.buffer.drain(..pos + 2);

                        if self.chunk_size == 0 {
                            // Last chunk
                            return Ok(true);
                        }

                        self.chunk_state = ChunkState::Data;
                    } else {
                        return Ok(false);
                    }
                }
                ChunkState::Data => {
                    if self.buffer.len() >= self.chunk_size {
                        request.body.extend_from_slice(&self.buffer[..self.chunk_size]);
                        self.buffer.drain(..self.chunk_size);
                        self.chunk_state = ChunkState::TrailingCRLF;
                    } else {
                        return Ok(false);
                    }
                }
                ChunkState::TrailingCRLF => {
                    if self.buffer.len() >= 2 {
                        self.buffer.drain(..2);
                        self.chunk_state = ChunkState::Size;
                    } else {
                        return Ok(false);
                    }
                }
            }
        }
    }

    fn find_crlf(&self) -> Option<usize> {
        self.buffer.windows(2).position(|w| w == b"\r\n")
    }

    pub fn is_complete(&self) -> bool {
        self.state == ParserState::Done
    }
}

pub fn parse_query_string(uri: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    
    if let Some(query_start) = uri.find('?') {
        let query = &uri[query_start + 1..];
        for pair in query.split('&') {
            if let Some(eq_pos) = pair.find('=') {
                let key = urldecode(&pair[..eq_pos]);
                let value = urldecode(&pair[eq_pos + 1..]);
                params.insert(key, value);
            }
        }
    }
    
    params
}

fn urldecode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    
    result
}