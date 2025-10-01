mod config;
mod epoll_handler;
mod http_parser;
mod http_response;
mod server;
mod cgi;
mod session;

use std::process;
use config::Config;
use server::Server;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() != 2 {
        eprintln!("Usage: {} <config_file>", args[0]);
        process::exit(1);
    }

    let config_path = &args[1];
    
    let config = match Config::from_file(config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            process::exit(1);
        }
    };

    let mut server = match Server::new(config) {
        Ok(srv) => srv,
        Err(e) => {
            eprintln!("Failed to create server: {}", e);
            process::exit(1);
        }
    };

    println!("Server starting...");
    
    if let Err(e) = server.run() {
        eprintln!("Server error: {}", e);
        process::exit(1);
    }
}