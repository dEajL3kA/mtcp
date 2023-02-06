/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */

use std::cell::Ref;
use std::io::{Result as IoResult, ErrorKind};
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;

use mio::{Token, Interest};
use mio::net::TcpListener as MioTcpListener;

use log::warn;

use crate::utilities::Timeout;
use crate::{TcpConnection, TcpManager, TcpError};
use crate::manager::{TcpPollContext};

/// A TCP socket server, listening for connections, akin to
/// [`std::net::TcpListener`](std::net::TcpListener)
///
/// All I/O operations provided by `mtcp_rs::TcpListener` are "blocking", but –
/// unlike the `std::net` implementation – proper ***timeout*** and
/// ***cancellation*** support is available. The `mtcp_rs::TcpListener` is tied
/// to an [`mtcp_rs::TcpManager`](crate::TcpManager) instance.
/// 
/// If the `timeout` parameter was set to `Some(Duration)` and if the I/O
/// operation does **not** complete before the specified timeout period
/// expires, then the pending I/O operation will be aborted and fail with an
/// [`TcpError::TimedOut`](crate::TcpError::TimedOut) error.
#[derive(Debug)]
pub struct TcpListener {
    listener: MioTcpListener,
    token: Token,
    manager: Rc<TcpManager>,
}

impl TcpListener {
    /// Creates a new `TcpListener` which will be bound to the specified socket address.
    /// 
    /// The new `TcpListener` is tied to the specified `TcpManager` instance.
    pub fn bind(manager: &Rc<TcpManager>, addr: SocketAddr) -> IoResult<Self> {
        let manager = manager.clone();
        let (listener, token) = Self::initialize(manager.context(), addr)?;

        Ok(Self {
            listener,
            token,
            manager,
        })
    }

    fn initialize(context: Ref<TcpPollContext>, addr: SocketAddr) -> IoResult<(MioTcpListener, Token)> {
        let mut listener = MioTcpListener::bind(addr)?;
        let token = context.token();
        context.registry().register(&mut listener, token, Interest::READABLE)?;
        Ok((listener, token))
    }

    /// Accept a new incoming TCP connection from this listener.
    /// 
    /// An optional ***timeout*** can be specified, after which the operation
    /// is going to fail, if there is **no** incoming connection yet. 
    pub fn accept(&self, timeout: Option<Duration>) -> Result<TcpConnection, TcpError> {
        if self.manager.cancelled() {
            return Err(TcpError::Cancelled);
        }

        let timeout = Timeout::start(timeout);

        match Self::event_accept(&self.listener) {
            Ok(Some(connection)) => return Ok(connection),
            Ok(_) => (),
            Err(error) => return Err(error.into()),
        }

        let mut context = self.manager.context_mut();

        loop {
            let remaining = timeout.remaining_time();
            match context.poll(remaining) {
                Ok(events) => {
                    for _event in events.iter().filter(|event| event.token() == self.token) {
                        match Self::event_accept(&self.listener) {
                            Ok(Some(connection)) => return Ok(connection),
                            Ok(_) => (),
                            Err(error) => return Err(error.into()),
                        }
                    }
                },
                Err(error) => return Err(error.into()),
            }
            if self.manager.cancelled() {
                return Err(TcpError::Cancelled);
            }
            if remaining.map(|time| time.is_zero()).unwrap_or(false) {
                return Err(TcpError::TimedOut);
            }
        }
    }

    fn event_accept(listener: &MioTcpListener) -> IoResult<Option<TcpConnection>> {
        loop {
            match listener.accept() {
                Ok((stream, _addr)) => return Ok(Some(TcpConnection::new(stream))),
                Err(error) => match error.kind() {
                    ErrorKind::Interrupted => (),
                    ErrorKind::WouldBlock => return Ok(None),
                    _ => return Err(error),
                },
            }
        }
    }

    /// Get the *local* socket address to which this `TcpListener` is bound.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.listener.local_addr().ok()
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        let context = self.manager.context();
        if let Err(error) = context.registry().deregister(&mut self.listener) {
            warn!("Failed to de-register: {:?}", error);
        }
    }
}
