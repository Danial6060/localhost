use std::collections::HashMap;
use std::fs;
use std::io;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub server_names: Vec<String>,
    pub error_pages: HashMap<u16, String>,
    pub client_max_body_size: usize,
    pub routes: Vec<Route>,
}

#[derive(Debug, Clone)]
pub struct Route {
    pub path: String,
    pub methods: Vec<String>,
    pub root: Option<String>,
    pub index: Vec<String>,
    pub autoindex: bool,
    pub redirect: Option<(u16, String)>,
    pub cgi_extension: Option<String>,
    pub cgi_path: Option<String>,
    pub upload_dir: Option<String>,
}

#[derive(Debug)]
pub struct Config {
    pub servers: Vec<ServerConfig>,
}

impl Config {
    pub fn from_file(path: &str) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    fn parse(content: &str) -> io::Result<Self> {
        let mut servers = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();
            
            if line.starts_with("server {") {
                let (server, next_idx) = Self::parse_server(&lines, i)?;
                servers.push(server);
                i = next_idx;
            } else {
                i += 1;
            }
        }

        if servers.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "No servers configured"));
        }

        // Validate no duplicate host:port combinations
        let mut seen = HashMap::new();
        for server in &servers {
            let key = format!("{}:{}", server.host, server.port);
            if seen.contains_key(&key) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Duplicate server configuration for {}", key)
                ));
            }
            seen.insert(key, true);
        }

        Ok(Config { servers })
    }

    fn parse_server(lines: &[&str], start: usize) -> io::Result<(ServerConfig, usize)> {
        let mut host = String::from("127.0.0.1");
        let mut port = 8080u16;
        let mut server_names = Vec::new();
        let mut error_pages = HashMap::new();
        let mut client_max_body_size = 1048576; // 1MB default
        let mut routes = Vec::new();
        let mut i = start + 1;

        while i < lines.len() {
            let line = lines[i].trim();

            if line == "}" {
                break;
            }

            if line.starts_with("listen ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let addr = parts[1].trim_end_matches(';');
                    if let Some(colon_pos) = addr.rfind(':') {
                        host = addr[..colon_pos].to_string();
                        port = addr[colon_pos + 1..].parse().unwrap_or(8080);
                    } else {
                        port = addr.parse().unwrap_or(8080);
                    }
                }
            } else if line.starts_with("server_name ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for name in &parts[1..] {
                    server_names.push(name.trim_end_matches(';').to_string());
                }
            } else if line.starts_with("error_page ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    if let Ok(code) = parts[1].parse::<u16>() {
                        error_pages.insert(code, parts[2].trim_end_matches(';').to_string());
                    }
                }
            } else if line.starts_with("client_max_body_size ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let size_str = parts[1].trim_end_matches(';');
                    client_max_body_size = Self::parse_size(size_str);
                }
            } else if line.starts_with("location ") {
                let (route, next_idx) = Self::parse_location(lines, i)?;
                routes.push(route);
                i = next_idx;
                continue;
            }

            i += 1;
        }

        // Add default route if none specified
        if routes.is_empty() {
            routes.push(Route {
                path: "/".to_string(),
                methods: vec!["GET".to_string(), "POST".to_string(), "DELETE".to_string()],
                root: Some("./www".to_string()),
                index: vec!["index.html".to_string()],
                autoindex: false,
                redirect: None,
                cgi_extension: None,
                cgi_path: None,
                upload_dir: None,
            });
        }

        Ok((ServerConfig {
            host,
            port,
            server_names,
            error_pages,
            client_max_body_size,
            routes,
        }, i + 1))
    }

    fn parse_location(lines: &[&str], start: usize) -> io::Result<(Route, usize)> {
        let line = lines[start].trim();
        let parts: Vec<&str> = line.split_whitespace().collect();
        let path = if parts.len() >= 2 {
            parts[1].trim_end_matches('{').trim().to_string()
        } else {
            "/".to_string()
        };

        let mut methods = vec!["GET".to_string(), "POST".to_string(), "DELETE".to_string()];
        let mut root = None;
        let mut index = vec!["index.html".to_string()];
        let mut autoindex = false;
        let mut redirect = None;
        let mut cgi_extension = None;
        let mut cgi_path = None;
        let mut upload_dir = None;
        let mut i = start + 1;

        while i < lines.len() {
            let line = lines[i].trim();

            if line == "}" {
                break;
            }

            if line.starts_with("allow_methods ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                methods = parts[1..].iter()
                    .map(|s| s.trim_end_matches(';').to_uppercase())
                    .collect();
            } else if line.starts_with("root ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    root = Some(parts[1].trim_end_matches(';').to_string());
                }
            } else if line.starts_with("index ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                index = parts[1..].iter()
                    .map(|s| s.trim_end_matches(';').to_string())
                    .collect();
            } else if line.starts_with("autoindex ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    autoindex = parts[1].trim_end_matches(';') == "on";
                }
            } else if line.starts_with("return ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    if let Ok(code) = parts[1].parse::<u16>() {
                        redirect = Some((code, parts[2].trim_end_matches(';').to_string()));
                    }
                }
            } else if line.starts_with("cgi_extension ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    cgi_extension = Some(parts[1].trim_end_matches(';').to_string());
                }
            } else if line.starts_with("cgi_path ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    cgi_path = Some(parts[1].trim_end_matches(';').to_string());
                }
            } else if line.starts_with("upload_dir ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    upload_dir = Some(parts[1].trim_end_matches(';').to_string());
                }
            }

            i += 1;
        }

        Ok((Route {
            path,
            methods,
            root,
            index,
            autoindex,
            redirect,
            cgi_extension,
            cgi_path,
            upload_dir,
        }, i + 1))
    }

    fn parse_size(size_str: &str) -> usize {
        let size_str = size_str.to_uppercase();
        let multiplier = if size_str.ends_with('K') {
            1024
        } else if size_str.ends_with('M') {
            1024 * 1024
        } else if size_str.ends_with('G') {
            1024 * 1024 * 1024
        } else {
            1
        };

        let num_str = size_str.trim_end_matches(|c: char| !c.is_numeric());
        num_str.parse::<usize>().unwrap_or(1048576) * multiplier
    }
}