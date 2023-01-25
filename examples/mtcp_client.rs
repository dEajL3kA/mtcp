/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::{str, rc::Rc, net::SocketAddr, time::Duration, num::NonZeroUsize};

use mtcp_rs::{TcpManager, TcpStream, TcpError};

use dns_lookup::lookup_host;
use lazy_static::lazy_static;
use log::{debug, info, warn, error};
use regex::bytes::Regex;

const REMOTE_HOST: &str = "www.example.com";
const PORT_NUMBER: u16 = 80;
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    /* Initialize the log output */
    env_logger::init_from_env(env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"));
    
    /* Print logo */
    info!("mtcp - Example HTTP Client [Version {}]", PKG_VERSION);

    /* Initialize TcpManager */
    let manager = TcpManager::instance().expect("Failed to obtain TcpManager instance!");

    /* Register Canceller with Ctrl+C handler */
    let canceller = manager.canceller().expect("Failed to create canceller!");
    ctrlc::set_handler(move || {
        warn!("Shutdown has been requested!");
        canceller.cancel().expect("Failed to cancel operation!");
    })
    .expect("Failed to register CTRL+C handler!");

    // Lookup ip address
    info!("Looking up IP address for server: {:?}", REMOTE_HOST);
    match lookup_host(REMOTE_HOST) {
        Ok(result) => {
            for ip_addr in result {
                connect(&manager, ip_addr, REMOTE_HOST);
            }
        }
        Err(error) => error!("Address lookup failed: {:?}", error),
    }

    /* Bye! */
    info!("That's it, goodbye!");
}

fn connect(manager: &Rc<TcpManager>, ip_addr: std::net::IpAddr, hostname: &str) {
    // Connect to the server
    info!("Connecting to server: {}:{}", ip_addr, PORT_NUMBER);
    let mut stream = match TcpStream::connect(manager, SocketAddr::new(ip_addr, PORT_NUMBER), Some(Duration::from_secs(10))) {
        Ok(stream) => {
            info!("Connected: {:?} -> {:?}", stream.local_addr(), stream.peer_addr());
            stream /* connected successfully */
        },
        Err(error) => {
            match error.get_ref().and_then(|inner| inner.downcast_ref::<TcpError>()) {
                Some(tcp_error) => error!("TcpError: {}", tcp_error),
                None => error!("Connect operation has failed: {:?}", error),
            }
            return;
        },
    };

    // Send HTTP reuqest
    let request = format!("GET / HTTP/1.1\r\nHost: {}\r\nAccept: */*\r\n\r\n", hostname);
    info!("Sending HTTP request to server...");
    match stream.write_all_timeout(&request.into_bytes()[..], Some(Duration::from_secs(15))) {
        Ok(_) => (),
        Err(error) => {
            match error.get_ref().and_then(|inner| inner.downcast_ref::<TcpError>()) {
                Some(tcp_error) => error!("TcpError: {}", tcp_error),
                None => error!("Write operation has failed: {:?}", error),
            }
            return;
        },
    }

    // Read HTTP response
    info!("Reading HTTP response from server server...");
    let mut buffer = Vec::with_capacity(4096);
    match stream.read_all_timeout(&mut buffer, Some(Duration::from_secs(15)), NonZeroUsize::new(4096), end_of_message) {
        Ok(_) => info!("Response: {:?}", str::from_utf8(&buffer[..])),
        Err(error) => {
            match error.get_ref().and_then(|inner| inner.downcast_ref::<TcpError>()) {
                Some(tcp_error) => error!("TcpError: {}", tcp_error),
                None => error!("Write operation has failed: {:?}", error),
            }
            return;
        },
    }
}

fn end_of_message(buffer: &[u8]) -> bool {
    lazy_static! {
        static ref END_OF_HEADER: Regex = Regex::new(r"\r\n\r\n").expect("Failed to create regex!");
        static ref CONTENT_LENGTH: Regex = Regex::new(r"(?i)\r\nContent-Length:\s*(\d+)").expect("Failed to create regex!");
    }

    // Is HTTP header completed?
    let header_len = match END_OF_HEADER.find(buffer) {
        Some(header) => header.end(),
        None => return false,
    };

    // Find the "Content-Length" header
    let bondy_len = CONTENT_LENGTH.captures(&buffer[..header_len])
        .and_then(|cap| cap.get(1))
        .and_then(|cap| str::from_utf8(cap.as_bytes()).ok())
        .and_then(|str| str.parse::<usize>().ok());


    // Is the body complete yet?
    debug!("Content-Length: {:?}", bondy_len);
    bondy_len.map(|len| buffer.len() >= len + header_len).unwrap_or(false)
}
