/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::num::NonZeroUsize;
use std::str;
use std::sync::Arc;
use std::thread::{self, ThreadId};
use std::time::Duration;

use mtcp_rs::{TcpManager, TcpListener, TcpConnection, TcpStream, TcpError};

use crossbeam_channel::Receiver;
use lazy_static::lazy_static;
use lazy_rc::LazyArc;
use log::{info, warn, error};
use regex::bytes::Regex;

static CPU_COUNT: LazyArc<usize> = LazyArc::empty();

const PORT_NUMBER: u16 = 8080;
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    /* Initialize the log output */
    env_logger::init_from_env(env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"));
    
    /* Print logo */
    info!("mtcp - Example HTTP Server [Version {}]", PKG_VERSION);

    /* Initialize TcpManager */
    let manager = TcpManager::instance().expect("Failed to obtain TcpManager instance!");

    /* Register Canceller with Ctrl+C handler */
    let canceller = manager.canceller().expect("Failed to create canceller!");
    ctrlc::set_handler(move || {
        warn!("Shutdown has been requested!");
        canceller.cancel().expect("Failed to cancel operation!");
    })
    .expect("Failed to register CTRL+C handler!");

    /* Bind TcpListener to local socket */
    let listener = match TcpListener::bind(&manager, SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), PORT_NUMBER)) {
        Ok(value) => value,
        Err(error) => return error!("Failed to bind TcpListener: {:?}", error),
    };

    /* Create Crossbeam channel */
    let (channel_tx, channel_rx) = crossbeam_channel::bounded::<TcpConnection>(256);

    /* Detect number of processors */
    let cpu_count = cpu_count();

    /* Create some worker threads to handle incoming connections */
    let mut threads = Vec::with_capacity(*cpu_count);
    for _n in 0..(*cpu_count) {
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
                match error {
                    TcpError::Cancelled=> error!("Accept operation was cancelled!"),
                    TcpError::TimedOut => error!("Accept operation timed out!"),
                    TcpError::Failed(inner) => error!("Accept operation failed: {:?}", inner),
                    TcpError::Incomplete | TcpError::TooBig => unreachable!(),
                }
                break; /*stop server after an error was encountered */
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
        request_buffer.clear();
    }
}

fn handle_request(mut stream: TcpStream, buffer: &mut Vec<u8>, thread_id: ThreadId) {
    /* Read request */
    match stream.read_all_timeout(buffer, Some(Duration::from_secs(15)), NonZeroUsize::new(4096), NonZeroUsize::new(1048576), end_of_message) {
        Ok(_) => {
            let request_str = str::from_utf8(&buffer[..]).unwrap_or("invalid request!");
            info!("[{:?}] Request: {:?}", thread_id, request_str);
        },
        Err(error) => {
            match error {
                TcpError::Cancelled=> error!("Read operation was cancelled!"),
                TcpError::TimedOut => error!("Read operation timed out!"),
                TcpError::Incomplete => error!("Read operation is incomplete!"),
                TcpError::TooBig => error!("Read operation failed, because request data is too big!"),
                TcpError::Failed(inner) => error!("Read operation failed: {:?}", inner),
            }
            return;
        }
    }

    /* Write response */
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<!DOCTYPE html>\r\n<title>Hello</title><h1>Hello world!</h1>";
    if let Err(error) = stream.write_all_timeout(response.as_bytes(), Some(Duration::from_secs(15))) {
        match error {
            TcpError::Cancelled=> error!("Write operation was cancelled!"),
            TcpError::TimedOut => error!("Write operation timed out!"),
            TcpError::Incomplete => error!("Write operation is incomplete!"),
            TcpError::Failed(inner) => error!("Write operation failed: {:?}", inner),
            TcpError::TooBig => unreachable!(),
        }
    }
}

fn end_of_message(buffer: &[u8]) -> bool {
    lazy_static! {
        static ref END_OF_MESSAGE: Regex = Regex::new(r"\r\n\r\n").expect("Failed to create regex!");
    }
    END_OF_MESSAGE.is_match(buffer)
}

fn cpu_count() -> Arc<usize> {
    CPU_COUNT.or_init_with(|| num_cpus::get().max(1))
}
