/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::net::SocketAddr;

use mio::net::TcpStream as MioTcpStream;

/// A pending incoming TCP connection, usually used to initialize a new [`mtcp_rs::TcpStream`](crate::TcpStream)
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

    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.stream.peer_addr().ok()
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.stream.local_addr().ok()
    }
}
