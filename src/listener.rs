/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */

use std::cell::Ref;
use std::io::{Result, ErrorKind};
use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;

use mio::{Token, Interest};
use mio::net::TcpListener as MioTcpListener;

use log::warn;

use crate::utilities::{set_up_timeout, compute_remaining_time};
use crate::{TcpConnection, TcpManager};
use crate::error::{ERROR_CANCELLED, ERROR_TIMEDOUT};
use crate::manager::{TcpPollContext};

/// A TCP socket server, listening for connections, akin to
/// [`std::net::TcpListener`](std::net::TcpListener)
///
/// All I/O operations provided by `mtcp_rs::TcpListener` are "blocking" by
/// default, but – unlike the `std::net` implementation – proper ***timeout***
/// and ***cancellation*** support is available. Each `mtcp_rs::TcpListener` is
/// tied to an [`mtcp_rs::TcpManager`](crate::TcpManager).
/// 
/// If the `timeout` parameter was set to `Some(Duration)` and if the I/O
/// operation does **not** complete before the specified timeout period
/// expires, then the pending I/O operation will fail as soon as possible with
/// an [`TcpError::TimedOut`](crate::TcpError::TimedOut) error.
#[derive(Debug)]
pub struct TcpListener {
    listener: MioTcpListener,
    token: Token,
    manager: Rc<TcpManager>,
}

impl TcpListener {
    pub fn bind(manager: &Rc<TcpManager>, addr: SocketAddr) -> Result<Self> {
        let manager = manager.clone();
        let (listener, token) = Self::initialize(manager.context(), addr)?;

        Ok(Self {
            listener,
            token,
            manager,
        })
    }

    fn initialize(context: Ref<TcpPollContext>, addr: SocketAddr) -> Result<(MioTcpListener, Token)> {
        let mut listener = MioTcpListener::bind(addr)?;
        let token = context.token();
        context.registry().register(&mut listener, token, Interest::READABLE)?;
        Ok((listener, token))
    }

    pub fn accept(&self, timeout: Option<Duration>) -> Result<TcpConnection> {
        if self.manager.cancelled() {
            return ERROR_CANCELLED.result();
        }

        let timeout = set_up_timeout(timeout);

        match Self::event_accept(&self.listener) {
            Ok(Some(connection)) => return Ok(connection),
            Ok(_) => (),
            Err(error) => return Err(error),
        }

        let mut context = self.manager.context_mut();

        loop {
            let remaining = timeout.map(compute_remaining_time);
            match context.poll(remaining) {
                Ok(events) => {
                    for _event in events.iter().filter(|event| event.token() == self.token) {
                        match Self::event_accept(&self.listener) {
                            Ok(Some(connection)) => return Ok(connection),
                            Ok(_) => (),
                            Err(error) => return Err(error),
                        }
                    }
                },
                Err(error) => return Err(error),
            }
            if self.manager.cancelled() {
                return ERROR_CANCELLED.result();
            }
            if remaining.map(|time| time.is_zero()).unwrap_or(false) {
                return ERROR_TIMEDOUT.result();
            }
        }
    }

    fn event_accept(listener: &MioTcpListener) -> Result<Option<TcpConnection>> {
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
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        let context = self.manager.context();
        if let Err(error) = context.registry().deregister(&mut self.listener) {
            warn!("Failed to de-register: {:?}", error);
        }
    }
}
