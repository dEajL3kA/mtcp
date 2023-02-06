/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::io::Result as IoResult;
use std::net::{SocketAddr, Shutdown};

use mio::net::TcpStream as MioTcpStream;

/// A pending incoming TCP connection, usually used to initialize a new
/// [`mtcp_rs::TcpStream`](crate::TcpStream)
/// 
/// Unlike an `mtcp_rs::TcpStream` instance, the `mtcp_rs::TcpConnection`
/// instance is **not** yet tied to a
/// [`mtcp_rs::TcpManager`](crate::TcpManager) instance and can therefore
/// safely be moved across the thread boundary.
#[derive(Debug)]
pub struct TcpConnection {
    stream: MioTcpStream,
}

impl TcpConnection {
    pub(crate) fn new(stream: MioTcpStream) -> Self {
        Self {
            stream,
        }
    }

    pub(crate) fn stream(self) -> MioTcpStream {
        self.stream
    }

    /// Get the *peer* socket address of this TCP connection.
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.stream.peer_addr().ok()
    }

    /// Get the *local* socket address of this TCP connection.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.stream.local_addr().ok()
    }

    /// Shuts down the read, write, or both halves of this TCP connection.
    pub fn shutdown(&self, how: Shutdown) -> IoResult<()> {
        self.stream.shutdown(how)
    }
}
