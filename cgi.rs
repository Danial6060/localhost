use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};

pub struct CgiHandler;

impl CgiHandler {
  pub fn execute(
    cgi_path: &str,
    script_path: &str,
    method: &str,
    query_string: &str,
    headers: &HashMap<String, String>,
    body: &[u8],
    server_addr: &str,
    server_port: u16,
    remote_addr: &str,
) -> Result<Vec<u8>, String> {
    // Create owned strings for environment variables
    let server_port_str = server_port.to_string();
    let content_length_str = body.len().to_string();
    
    let mut env_vars: HashMap<&str, &str> = HashMap::new();

    // Set CGI environment variables
    env_vars.insert("GATEWAY_INTERFACE", "CGI/1.1");
    env_vars.insert("SERVER_PROTOCOL", "HTTP/1.1");
    env_vars.insert("SERVER_SOFTWARE", "Webserv/1.0");
    env_vars.insert("REQUEST_METHOD", method);
    env_vars.insert("QUERY_STRING", query_string);
    env_vars.insert("SCRIPT_FILENAME", script_path);
    env_vars.insert("SCRIPT_NAME", script_path);
    env_vars.insert("SERVER_NAME", server_addr);
    env_vars.insert("SERVER_PORT", &server_port_str);
    env_vars.insert("REMOTE_ADDR", remote_addr);

    // Set PATH_INFO
    if let Some(info_start) = script_path.rfind('.') {
        if let Some(slash_after) = script_path[info_start..].find('/') {
            let path_info = &script_path[info_start + slash_after..];
            env_vars.insert("PATH_INFO", path_info);
        }
    }

    // Content-related headers
    if let Some(content_type) = headers.get("content-type") {
        env_vars.insert("CONTENT_TYPE", content_type);
    }
    
    if !body.is_empty() {
        env_vars.insert("CONTENT_LENGTH", &content_length_str);
    }

    // Pass other headers as HTTP_*
    let mut http_headers: Vec<(String, String)> = Vec::new();
    for (key, value) in headers {
        let env_key = format!("HTTP_{}", key.to_uppercase().replace('-', "_"));
        http_headers.push((env_key, value.clone()));
    }

    // Get directory of script for proper relative path handling
    let script_dir = std::path::Path::new(script_path)
        .parent()
        .unwrap_or(std::path::Path::new("."));

    // Execute CGI
    let mut cmd = Command::new(cgi_path);
    cmd.arg(script_path)
        .current_dir(script_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Add base environment variables
    for (k, v) in env_vars.iter() {
        cmd.env(k, v);
    }

    // Add HTTP headers
    for (k, v) in http_headers.iter() {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn()
        .map_err(|e| format!("Failed to spawn CGI process: {}", e))?;

    // Write body to stdin
    if !body.is_empty() {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(body)
                .map_err(|e| format!("Failed to write to CGI stdin: {}", e))?;
        }
    }

    // Read output with timeout
    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to read CGI output: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "CGI script failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(output.stdout)
}

    pub fn parse_cgi_output(output: &[u8]) -> Result<(HashMap<String, String>, Vec<u8>), String> {
        let mut headers = HashMap::new();
        let output_str = std::str::from_utf8(output)
            .map_err(|_| "Invalid UTF-8 in CGI output")?;

        // Find end of headers
        if let Some(pos) = output_str.find("\r\n\r\n") {
            let header_section = &output_str[..pos];
            let body = &output[pos + 4..];

            for line in header_section.lines() {
                if let Some(colon_pos) = line.find(':') {
                    let key = line[..colon_pos].trim().to_lowercase();
                    let value = line[colon_pos + 1..].trim().to_string();
                    headers.insert(key, value);
                }
            }

            Ok((headers, body.to_vec()))
        } else if let Some(pos) = output_str.find("\n\n") {
            let header_section = &output_str[..pos];
            let body = &output[pos + 2..];

            for line in header_section.lines() {
                if let Some(colon_pos) = line.find(':') {
                    let key = line[..colon_pos].trim().to_lowercase();
                    let value = line[colon_pos + 1..].trim().to_string();
                    headers.insert(key, value);
                }
            }

            Ok((headers, body.to_vec()))
        } else {
            // No headers, all body
            Ok((headers, output.to_vec()))
        }
    }
}