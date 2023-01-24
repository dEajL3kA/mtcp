/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroUsize;
use std::str;
use std::thread::{self, ThreadId};
use std::time::Duration;

use mtcp_rs::{TcpManager, TcpListener, TcpConnection, TcpStream, TcpError};

use crossbeam_channel::Receiver;
use lazy_static::lazy_static;
use log::{info, warn, error};
use regex::bytes::Regex;

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    /* Initialize the log output */
    env_logger::init_from_env(env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"));
    
    /* Print logo */
    info!("mtcp - Example HTTP Server [Version {}]", PKG_VERSION);

    /* Initialize TcpManager */
    let manager = TcpManager::instance().expect("Failed to obtain TcpManager instance!");

    /* Register Canceller with Ctrl+C handler */
    let canceller = manager.canceller();
    ctrlc::set_handler(move || {
        warn!("Shutdown has been requested!");
        canceller.cancel().expect("Failed to cancel operation!");
    })
    .expect("Failed to register CTRL+C handler!");

    /* Bind TcpListener to local socket */
    let listener = TcpListener::bind(&manager, SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080)).expect("Failed to bin TcpListener!");

    /* Create Crossbeam channel */
    let (channel_tx, channel_rx) = crossbeam_channel::bounded::<TcpConnection>(256);

    /* Create some worker threads to handle connections */
    let mut threads = Vec::new();
    for _n in 0..5 {
        let thread_receiver = channel_rx.clone();
        threads.push(thread::spawn(move || thread_worker(thread_receiver)));
    }

    /* Accept all incoming connections */
    info!("Waiting for incoming connections...");
    loop {
        match listener.accept(Some(Duration::from_secs(30))) {
            Ok(connection) => {
                info!("Connection received: {:?} -> {:?}", connection.local_addr(), connection.peer_addr());
                if let Err(error) = channel_tx.send_timeout(connection, Duration::from_secs(15)) {
                    warn!("Failed to enqueue the connection: {:?}", error);
                }
            },
            Err(error) => {
                match error.get_ref().and_then(|inner| inner.downcast_ref::<TcpError>()) {
                    Some(tcp_error) => error!("TcpError: {}", tcp_error),
                    None => error!("Accept operation has failed: {:?}", error),
                }
                break;
            },
        }
    }

    /* Close the "sender" end of the channel*/
    drop(channel_tx);

    /* Wait for all worker threads to complete */
    threads.drain(..).for_each(|thread| thread.join().expect("Failed to join with worker thread!"));

    /* Bye! */
    info!("That's it, goodbye!");
}

fn thread_worker(receiver: Receiver<TcpConnection>) {
    /* Get thread id*/
    let thread_id = thread::current().id();

    /* Initialize TcpManager */
    let manager = TcpManager::instance().expect("Failed to obtain TcpManager instance!");

    /* Create buffer */
    let mut request_buffer: Vec<u8> = Vec::with_capacity(4096);

    /* Process all incoming connections */
    loop {
        match receiver.recv() {
            Ok(connection) => match TcpStream::from(&manager, connection) {
                Ok(stream) => handle_request(stream, &mut request_buffer, thread_id),
                Err(error) => warn!("[{:?}] Failed to init TcpStream: {:?}", thread_id, error),
            }
            Err(_) => break, /* channel is closed */
        }
    }
}

fn handle_request(mut stream: TcpStream, buffer: &mut Vec<u8>, thread_id: ThreadId) {
    /* Read request */
    match stream.read_all_timeout(buffer, Some(Duration::from_secs(15)), NonZeroUsize::new(4096), end_of_message) {
        Ok(_) => {
            let request_str = str::from_utf8(&buffer[..]).unwrap_or("invalid request!");
            info!("[{:?}] Request: {:?}", thread_id, request_str);
        },
        Err(error) => {
            warn!("[{:?}] The read operation has failed: {:?}", thread_id, error);
            return;
        }
    }

    /* Write response */
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\nHello!\r\n";
    if let Err(error) = stream.write_all_timeout(response.as_bytes(), Some(Duration::from_secs(15))) {
        error!("[{:?}] Failed to write response: {:?}", thread_id, error);
    }

    /* Clear the buffer for next time */
    buffer.clear();
}

fn end_of_message(buffer: &[u8]) -> bool {
    lazy_static! {
        static ref END_OF_MESSAGE: Regex = Regex::new(r"\r\n\r\n").expect("Failed to create regular expression!");
    }
    END_OF_MESSAGE.is_match(buffer)
}
