use crate::config::{Config, Route, ServerConfig};
use crate::epoll_handler::{set_nonblocking, Epoll};
use crate::http_parser::{HttpParser, HttpRequest};
use crate::http_response::HttpResponse;
use crate::cgi::CgiHandler;
use crate::session::{SessionManager, parse_cookies, create_set_cookie};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::{Duration, Instant};

const MAX_EVENTS: usize = 1024;
const BUFFER_SIZE: usize = 8192;
const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);

enum ClientState {
    Reading,
    Writing { response: Vec<u8>, written: usize },
}

struct Client {
    stream: TcpStream,
    state: ClientState,
    parser: HttpParser,
    request: HttpRequest,
    last_activity: Instant,
    server_config: ServerConfig,
}

pub struct Server {
    config: Config,
    epoll: Epoll,
    listeners: Vec<TcpListener>,
    clients: HashMap<RawFd, Client>,
    session_manager: SessionManager,
}

impl Server {
    pub fn new(config: Config) -> io::Result<Self> {
        let epoll = Epoll::new()?;
        let mut listeners = Vec::new();

        // Create listeners for each server
        for server_config in &config.servers {
            let addr = format!("{}:{}", server_config.host, server_config.port);
            let listener = TcpListener::bind(&addr)?;
            
            set_nonblocking(listener.as_raw_fd())?;
            
            // Register listener with epoll
            epoll.add(
                listener.as_raw_fd(),
                libc::EPOLLIN as u32,
                listener.as_raw_fd() as u64,
            )?;

            println!("Listening on {}", addr);
            listeners.push(listener);
        }

        Ok(Server {
            config,
            epoll,
            listeners,
            clients: HashMap::new(),
            session_manager: SessionManager::new(),
        })
    }

    pub fn run(&mut self) -> io::Result<()> {
        let mut events = vec![
            libc::epoll_event {
                events: 0,
                u64: 0,
            };
            MAX_EVENTS
        ];

        loop {
            // Cleanup expired sessions periodically
            self.session_manager.cleanup_expired(3600); // 1 hour

            // Epoll wait with timeout for connection management
            let n_events = match self.epoll.wait(&mut events, 1000) {
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };

            // Check for timeouts
            self.check_timeouts();

            for i in 0..n_events {
                let fd = events[i].u64 as RawFd;
                let event_flags = events[i].events;

                // Check if it's a listener
                if self.is_listener(fd) {
                    self.accept_connection(fd)?;
                } else if self.clients.contains_key(&fd) {
                    if event_flags & libc::EPOLLIN as u32 != 0 {
                        if let Err(_) = self.handle_read(fd) {
                            self.close_client(fd);
                        }
                    } else if event_flags & libc::EPOLLOUT as u32 != 0 {
                        if let Err(_) = self.handle_write(fd) {
                            self.close_client(fd);
                        }
                    }

                    if event_flags & (libc::EPOLLERR | libc::EPOLLHUP) as u32 != 0 {
                        self.close_client(fd);
                    }
                }
            }
        }
    }

    fn is_listener(&self, fd: RawFd) -> bool {
        self.listeners.iter().any(|l| l.as_raw_fd() == fd)
    }

    fn accept_connection(&mut self, listener_fd: RawFd) -> io::Result<()> {
        let listener = self.listeners
            .iter()
            .find(|l| l.as_raw_fd() == listener_fd)
            .unwrap();

        loop {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    set_nonblocking(stream.as_raw_fd())?;

                    let fd = stream.as_raw_fd();

                    // Find matching server config
                    let server_config = self.find_server_config(listener_fd);

                    let client = Client {
                        stream,
                        state: ClientState::Reading,
                        parser: HttpParser::new(),
                        request: HttpRequest::new(),
                        last_activity: Instant::now(),
                        server_config,
                    };

                    self.epoll.add(fd, libc::EPOLLIN as u32, fd as u64)?;
                    self.clients.insert(fd, client);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    fn find_server_config(&self, listener_fd: RawFd) -> ServerConfig {
        let listener = self.listeners
            .iter()
            .find(|l| l.as_raw_fd() == listener_fd)
            .unwrap();

        let addr = listener.local_addr().unwrap();

        // Find first matching server config
        for server in &self.config.servers {
            if server.port == addr.port() {
                return server.clone();
            }
        }

        self.config.servers[0].clone()
    }

    fn handle_read(&mut self, fd: RawFd) -> io::Result<()> {
        let client = self.clients.get_mut(&fd).unwrap();
        client.last_activity = Instant::now();

        let mut buffer = [0u8; BUFFER_SIZE];
        
        match client.stream.read(&mut buffer) {
            Ok(0) => {
                // Connection closed
                return Err(io::Error::new(io::ErrorKind::ConnectionReset, "Connection closed"));
            }
            Ok(n) => {
                // Parse the request
                if let Err(e) = client.parser.parse(&buffer[..n], &mut client.request) {
                    let response = HttpResponse::error_page(400, None);
                    self.send_response(fd, response)?;
                    return Ok(());
                }

                // Check if request is complete
                if client.request.complete {
                    self.process_request(fd)?;
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // No data available, continue
            }
            Err(e) => return Err(e),
        }

        Ok(())
    }

    fn handle_write(&mut self, fd: RawFd) -> io::Result<()> {
        let client = self.clients.get_mut(&fd).unwrap();
        client.last_activity = Instant::now();

        if let ClientState::Writing { ref response, ref mut written } = client.state {
            match client.stream.write(&response[*written..]) {
                Ok(0) => {
                    return Err(io::Error::new(io::ErrorKind::WriteZero, "Write zero"));
                }
                Ok(n) => {
                    *written += n;

                    if *written >= response.len() {
                        // Response sent, reset for next request
                        client.state = ClientState::Reading;
                        client.parser = HttpParser::new();
                        client.request = HttpRequest::new();

                        // Switch back to reading
                        self.epoll.modify(fd, libc::EPOLLIN as u32, fd as u64)?;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Can't write now, will try again
                }
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

   fn process_request(&mut self, fd: RawFd) -> io::Result<()> {
    // Clone the data we need before borrowing self mutably
    let (method, uri, body_len, server_config) = {
        let client = self.clients.get(&fd).unwrap();
        (
            client.request.method.clone(),
            client.request.uri.clone(),
            client.request.body.len(),
            client.server_config.clone(),
        )
    };

    // Check body size limit
    if body_len > server_config.client_max_body_size {
        let response = HttpResponse::error_page(
            413,
            server_config.error_pages.get(&413).map(|s| s.as_str()),
        );
        return self.send_response(fd, response);
    }

    // Find matching route
    let route = self.find_route(&uri, &server_config);

    // Check if method is allowed
    if let Some(ref route) = route {
        if !route.methods.contains(&method) {
            let response = HttpResponse::error_page(
                405,
                server_config.error_pages.get(&405).map(|s| s.as_str()),
            );
            return self.send_response(fd, response);
        }
    }

    // Handle redirect
    if let Some(ref route) = route {
        if let Some((code, ref location)) = route.redirect {
            let mut response = HttpResponse::new(code);
            response.add_header("Location".to_string(), location.clone());
            return self.send_response(fd, response);
        }
    }

    // Process based on method
    match method.as_str() {
        "GET" => self.handle_get(fd, route),
        "POST" => self.handle_post(fd, route),
        "DELETE" => self.handle_delete(fd, route),
        _ => {
            let response = HttpResponse::error_page(
                405,
                server_config.error_pages.get(&405).map(|s| s.as_str()),
            );
            self.send_response(fd, response)
        }
    }
}

    fn handle_get(&mut self, fd: RawFd, route: Option<&Route>) -> io::Result<()> {
        let client = self.clients.get(&fd).unwrap();
        let request = &client.request;
        let server_config = &client.server_config;

        let route = match route {
            Some(r) => r,
            None => {
                let response = HttpResponse::error_page(
                    404,
                    server_config.error_pages.get(&404).map(|s| s.as_str()),
                );
                return self.send_response(fd, response);
            }
        };

        let uri_path = request.uri.split('?').next().unwrap_or(&request.uri);
        let file_path = self.resolve_path(uri_path, route);

        // Check if file exists
        let metadata = match std::fs::metadata(&file_path) {
            Ok(m) => m,
            Err(_) => {
                let response = HttpResponse::error_page(
                    404,
                    server_config.error_pages.get(&404).map(|s| s.as_str()),
                );
                return self.send_response(fd, response);
            }
        };

        // If directory
       // If directory
if metadata.is_dir() {
    // Try index files
    for index_file in &route.index {
        let index_path = format!("{}/{}", file_path, index_file);
        if std::path::Path::new(&index_path).exists() {
            return self.serve_file(fd, &index_path);
        }
    }

    // Directory listing
    if route.autoindex {
        // Clone what we need before calling serve_directory_listing
        let file_path_clone = file_path.clone();
        let uri_path_clone = uri_path.to_string();
        return self.serve_directory_listing(fd, &file_path_clone, &uri_path_clone);
    } else {
        let response = HttpResponse::error_page(
            403,
            server_config.error_pages.get(&403).map(|s| s.as_str()),
        );
        return self.send_response(fd, response);
    }
}

        // Check for CGI
        if let Some(ref cgi_ext) = route.cgi_extension {
            if file_path.ends_with(cgi_ext) {
                return self.execute_cgi(fd, route, &file_path);
            }
        }

        // Serve regular file
        self.serve_file(fd, &file_path)
    }

    fn handle_post(&mut self, fd: RawFd, route: Option<&Route>) -> io::Result<()> {
        let client = self.clients.get(&fd).unwrap();
        let request = &client.request;
        let server_config = &client.server_config;

        let route = match route {
            Some(r) => r,
            None => {
                let response = HttpResponse::error_page(
                    404,
                    server_config.error_pages.get(&404).map(|s| s.as_str()),
                );
                return self.send_response(fd, response);
            }
        };

        let uri_path = request.uri.split('?').next().unwrap_or(&request.uri);

        // Check for file upload
        if let Some(content_type) = request.headers.get("content-type") {
            if content_type.contains("multipart/form-data") {
                return self.handle_file_upload(fd, route);
            }
        }

        // Check for CGI
        let file_path = self.resolve_path(uri_path, route);
        if let Some(ref cgi_ext) = route.cgi_extension {
            if uri_path.ends_with(cgi_ext) {
                return self.execute_cgi(fd, route, &file_path);
            }
        }

        // Default POST response
        let mut response = HttpResponse::new(200);
        response.set_body_str("POST request received");
        self.send_response(fd, response)
    }

    fn handle_delete(&mut self, fd: RawFd, route: Option<&Route>) -> io::Result<()> {
        let client = self.clients.get(&fd).unwrap();
        let request = &client.request;
        let server_config = &client.server_config;

        let route = match route {
            Some(r) => r,
            None => {
                let response = HttpResponse::error_page(
                    404,
                    server_config.error_pages.get(&404).map(|s| s.as_str()),
                );
                return self.send_response(fd, response);
            }
        };

        let uri_path = request.uri.split('?').next().unwrap_or(&request.uri);
        let file_path = self.resolve_path(uri_path, route);

        match std::fs::remove_file(&file_path) {
            Ok(_) => {
                let response = HttpResponse::new(204);
                self.send_response(fd, response)
            }
            Err(_) => {
                let response = HttpResponse::error_page(
                    404,
                    server_config.error_pages.get(&404).map(|s| s.as_str()),
                );
                self.send_response(fd, response)
            }
        }
    }

    fn serve_file(&mut self, fd: RawFd, file_path: &str) -> io::Result<()> {
        let content = match std::fs::read(file_path) {
            Ok(c) => c,
            Err(_) => {
                let client = self.clients.get(&fd).unwrap();
                let response = HttpResponse::error_page(
                    404,
                    client.server_config.error_pages.get(&404).map(|s| s.as_str()),
                );
                return self.send_response(fd, response);
            }
        };

        let mut response = HttpResponse::new(200);
        let content_type = self.get_content_type(file_path);
        response.add_header("Content-Type".to_string(), content_type);
        response.set_body(content);

        self.send_response(fd, response)
    }

    fn serve_directory_listing(&mut self, fd: RawFd, dir_path: &str, uri_path: &str) -> io::Result<()> {
        let entries = match std::fs::read_dir(dir_path) {
            Ok(entries) => entries,
            Err(_) => {
                let client = self.clients.get(&fd).unwrap();
                let response = HttpResponse::error_page(
                    500,
                    client.server_config.error_pages.get(&500).map(|s| s.as_str()),
                );
                return self.send_response(fd, response);
            }
        };

        let mut file_names = Vec::new();
        for entry in entries {
            if let Ok(entry) = entry {
                if let Some(name) = entry.file_name().to_str() {
                    file_names.push(name.to_string());
                }
            }
        }

        file_names.sort();

        let response = HttpResponse::directory_listing(dir_path, uri_path, file_names);
        self.send_response(fd, response)
    }

    fn execute_cgi(&mut self, fd: RawFd, route: &Route, script_path: &str) -> io::Result<()> {
    let client = self.clients.get(&fd).unwrap();
    let request = &client.request;
    let server_config = &client.server_config;

    let cgi_path = route.cgi_path.as_ref().map(|s| s.as_str()).unwrap_or("/usr/bin/python3");
    let query_string = request.uri.split('?').nth(1).unwrap_or("");

    // ADD THIS DEBUG LINE
    eprintln!("DEBUG: Executing CGI: {} {}", cgi_path, script_path);

    let remote_addr = client.stream.peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "0.0.0.0".to_string());

    match CgiHandler::execute(
        cgi_path,
        script_path,
        &request.method,
        query_string,
        &request.headers,
        &request.body,
        &server_config.host,
        server_config.port,
        &remote_addr,
    ) {
        Ok(output) => {
            // ADD THIS DEBUG LINE
            eprintln!("DEBUG: CGI output length: {}", output.len());
            
            match CgiHandler::parse_cgi_output(&output) {
                Ok((cgi_headers, body)) => {
                    // ADD THIS DEBUG LINE
                    eprintln!("DEBUG: CGI parsed successfully");
                    
                    let status_code = cgi_headers
                        .get("status")
                        .and_then(|s| s.split_whitespace().next())
                        .and_then(|s| s.parse::<u16>().ok())
                        .unwrap_or(200);

                    let mut response = HttpResponse::new(status_code);

                    for (key, value) in cgi_headers {
                        if key != "status" {
                            response.add_header(key, value);
                        }
                    }

                    if !response.headers.contains_key("Content-Type") {
                        response.add_header("Content-Type".to_string(), "text/html".to_string());
                    }

                    response.set_body(body);
                    self.send_response(fd, response)
                }
                Err(e) => {
                    // ADD THIS DEBUG LINE
                    eprintln!("DEBUG: CGI parse error: {}", e);
                    
                    let response = HttpResponse::error_page(
                        500,
                        server_config.error_pages.get(&500).map(|s| s.as_str()),
                    );
                    self.send_response(fd, response)
                }
            }
        }
        Err(e) => {
            // ADD THIS DEBUG LINE
            eprintln!("DEBUG: CGI execute error: {}", e);
            
            let response = HttpResponse::error_page(
                500,
                server_config.error_pages.get(&500).map(|s| s.as_str()),
            );
            self.send_response(fd, response)
        }
    }
}
    fn handle_file_upload(&mut self, fd: RawFd, route: &Route) -> io::Result<()> {
        let client = self.clients.get(&fd).unwrap();
        let request = &client.request;
        let server_config = &client.server_config;

        let upload_dir = route.upload_dir.as_ref().map(|s| s.as_str()).unwrap_or("./uploads");

        // Create upload directory if it doesn't exist
        std::fs::create_dir_all(upload_dir).ok();

        // Parse multipart data (simplified)
        if let Some(content_type) = request.headers.get("content-type") {
            if let Some(boundary_start) = content_type.find("boundary=") {
                let boundary = &content_type[boundary_start + 9..];
                let _boundary_marker = format!("--{}", boundary);
                // Simple file save (proper multipart parsing would be more complex)
                let filename = format!("{}/upload_{}.bin", upload_dir, std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs());

                if let Err(_) = std::fs::write(&filename, &request.body) {
                    let response = HttpResponse::error_page(
                        500,
                        server_config.error_pages.get(&500).map(|s| s.as_str()),
                    );
                    return self.send_response(fd, response);
                }

                let mut response = HttpResponse::new(201);
                response.set_body_str(&format!("File uploaded successfully: {}", filename));
                return self.send_response(fd, response);
            }
        }

        let mut response = HttpResponse::new(200);
        response.set_body_str("Upload processed");
        self.send_response(fd, response)
    }

    fn send_response(&mut self, fd: RawFd, mut response: HttpResponse) -> io::Result<()> {
        let client = self.clients.get_mut(&fd).unwrap();

        // Handle cookies and sessions
        if let Some(cookie_header) = client.request.headers.get("cookie") {
            let cookies = parse_cookies(cookie_header);
            
            if let Some(session_id) = cookies.get("sessionid") {
                // Session exists, update it
                if self.session_manager.get_session(session_id).is_none() {
                    // Create new session if old one expired
                    let new_session_id = self.session_manager.create_session();
                    response.add_header(
                        "Set-Cookie".to_string(),
                        create_set_cookie("sessionid", &new_session_id, Some(3600)),
                    );
                }
            } else {
                // No session, create one
                let session_id = self.session_manager.create_session();
                response.add_header(
                    "Set-Cookie".to_string(),
                    create_set_cookie("sessionid", &session_id, Some(3600)),
                );
            }
        } else {
            // No cookies at all, create session
            let session_id = self.session_manager.create_session();
            response.add_header(
                "Set-Cookie".to_string(),
                create_set_cookie("sessionid", &session_id, Some(3600)),
            );
        }

        let response_bytes = response.to_bytes();

        client.state = ClientState::Writing {
            response: response_bytes,
            written: 0,
        };

        // Switch to write mode
        self.epoll.modify(fd, libc::EPOLLOUT as u32, fd as u64)?;

        Ok(())
    }

    fn find_route<'a>(&self, uri: &str, config: &'a ServerConfig) -> Option<&'a Route> {
        let uri_path = uri.split('?').next().unwrap_or(uri);

        // Find longest matching route
        let mut best_match: Option<&Route> = None;
        let mut best_len = 0;

        for route in &config.routes {
            if uri_path.starts_with(&route.path) && route.path.len() > best_len {
                best_match = Some(route);
                best_len = route.path.len();
            }
        }

        best_match
    }

    fn resolve_path(&self, uri_path: &str, route: &Route) -> String {
        let root = route.root.as_ref().map(|s| s.as_str()).unwrap_or(".");
        
        // Remove route prefix from URI
        let relative_path = if uri_path.starts_with(&route.path) {
            &uri_path[route.path.len()..]
        } else {
            uri_path
        };

        let relative_path = relative_path.trim_start_matches('/');

        if relative_path.is_empty() {
            root.to_string()
        } else {
            format!("{}/{}", root, relative_path)
        }
    }

    fn get_content_type(&self, file_path: &str) -> String {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match extension {
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "js" => "application/javascript",
            "json" => "application/json",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "pdf" => "application/pdf",
            "txt" => "text/plain",
            _ => "application/octet-stream",
        }.to_string()
    }

    fn check_timeouts(&mut self) {
        let now = Instant::now();
        let mut to_close = Vec::new();

        for (fd, client) in &self.clients {
            if now.duration_since(client.last_activity) > CLIENT_TIMEOUT {
                to_close.push(*fd);
            }
        }

        for fd in to_close {
            self.close_client(fd);
        }
    }

    fn close_client(&mut self, fd: RawFd) {
        if let Some(client) = self.clients.remove(&fd) {
            let _ = self.epoll.delete(fd);
            drop(client.stream);
        }
    }
}