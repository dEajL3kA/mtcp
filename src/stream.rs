/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::io::{Read, Write, Result as IoResult, ErrorKind};
use std::net::{SocketAddr, Shutdown};
use std::num::NonZeroUsize;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::time::{Duration};

use mio::{Token, Interest};
use mio::net::TcpStream as MioTcpStream;

use log::warn;

use crate::utilities::Timeout;
use crate::{TcpConnection, TcpManager, TcpError};
use crate::manager::TcpPollContext;

/// A TCP stream between a local and a remote socket, akin to
/// [`std::net::TcpStream`](std::net::TcpStream)
///
/// All I/O operations provided by `mtcp_rs::TcpStream` are "blocking", but –
/// unlike the `std::net` implementation – proper ***timeout*** and
/// ***cancellation*** support is available. The `mtcp_rs::TcpStream` is tied
/// to an [`mtcp_rs::TcpManager`](crate::TcpManager) instance.
/// 
/// The TCP stream is created by [`connect()`](TcpStream::connect())ing to a
/// remote host, or directly [`from()`](TcpStream::from()) an existing
/// [`mtcp_rs::TcpConnection`](crate::TcpConnection).
/// 
/// If the `timeout` parameter was set to `Some(Duration)` and if the I/O
/// operation does **not** complete before the specified timeout period
/// expires, then the pending I/O operation will be aborted and fail with an
/// [`TcpError::TimedOut`](crate::TcpError::TimedOut) error.
/// 
/// Functions like [`Read::read()`](std::io::Read::read()) and
/// [`Write::write()`](std::io::Write::write()), which do **not** have an
/// explicit `timeout` parameter, *implicitly* use the timeouts that have been
/// set up via the
/// [`set_default_timeouts()`](TcpStream::set_default_timeouts()) function.
/// Initially, these timeouts are disabled.
#[derive(Debug)]
pub struct TcpStream {
    stream: MioTcpStream,
    token: Token,
    timeouts: (Option<Duration>, Option<Duration>),
    manager: Rc<TcpManager>,
}

impl TcpStream {
    pub fn from(manager: &Rc<TcpManager>, connection: TcpConnection) -> IoResult<Self> {
        let mut stream = connection.stream();
        let manager = manager.clone();
        let token = Self::register(&manager.context(), &mut stream)?;

        Ok(Self {
            stream,
            token,
            timeouts: (None, None),
            manager,
        })
    }

    pub fn set_default_timeouts(&mut self, timeout_rd: Option<Duration>, timeout_wr: Option<Duration>) {
        self.timeouts = (timeout_rd, timeout_wr);
    }

    pub fn peer_addr(&self) -> Option<SocketAddr> {
        self.stream.peer_addr().ok()
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.stream.local_addr().ok()
    }

    pub fn shutdown(&self, how: Shutdown) -> IoResult<()> {
        self.stream.shutdown(how)
    }

    fn register<T>(context: &T, stream: &mut MioTcpStream) -> IoResult<Token>
    where
        T: Deref<Target=TcpPollContext>
    {
        let token = context.token();
        context.registry().register(stream, token, Interest::READABLE | Interest::WRITABLE)?;
        Ok(token)
    }

    fn deregister<T>(context: &T, stream: &mut MioTcpStream)
    where
        T: Deref<Target=TcpPollContext>
    {
        if let Err(error) = context.registry().deregister(stream) {
            warn!("Failed to de-register: {:?}", error);
        }
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~
    // Connect functions
    // ~~~~~~~~~~~~~~~~~~~~~~~

    pub fn connect(manager: &Rc<TcpManager>, addr: SocketAddr, timeout: Option<Duration>) -> Result<Self, TcpError> {
        if manager.cancelled() {
            return Err(TcpError::Cancelled);
        }

        let mut stream = MioTcpStream::connect(addr)?;
        let manager = manager.clone();
        let token = Self::init_connection(&manager, &mut stream, timeout)?;

        Ok(Self {
            stream,
            token,
            timeouts: (None, None),
            manager,
        })
    }

    fn init_connection(manager: &Rc<TcpManager>, stream: &mut MioTcpStream, timeout: Option<Duration>) -> Result<Token, TcpError> {
        let mut context = manager.context_mut();
        let token = Self::register(&context, stream)?;

        match Self::await_connected(manager, &mut context, stream, token, timeout) {
            Ok(_) => Ok(token),
            Err(error) => {
                Self::deregister(&context, stream);
                Err(error)
            },
        }
    }

    fn await_connected<T>(manager: &Rc<TcpManager>, context: &mut T, stream: &mut MioTcpStream, token: Token, timeout: Option<Duration>) -> Result<(), TcpError>
    where
        T: DerefMut<Target=TcpPollContext>
    {
        let timeout = Timeout::start(timeout);

        loop {
            let remaining = timeout.remaining_time();
            match context.poll(remaining) {
                Ok(events) => {
                    for _event in events.iter().filter(|event| (event.token() == token)) {
                        match Self::event_conn(stream) {
                            Ok(true) => return Ok(()),
                            Ok(_) => (),
                            Err(error) => return Err(error.into()),
                        }
                    }
                },
                Err(error) => return Err(error.into()),
            }
            if manager.cancelled() {
                return Err(TcpError::Cancelled);
            }
            if remaining.map(|time| time.is_zero()).unwrap_or(false) {
                return Err(TcpError::TimedOut);
            }
        }
    }

    fn event_conn(stream: &mut MioTcpStream) -> IoResult<bool> {
        loop {
            if let Some(err) = stream.take_error()? {
                return Err(err);
            }
            match stream.peer_addr() {
                Ok(_addr) => return Ok(true),
                Err(error) => match error.kind() {
                    ErrorKind::Interrupted => (),
                    ErrorKind::NotConnected => return Ok(false),
                    _ => return Err(error),
                },
            }
        }
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~
    // Read functions
    // ~~~~~~~~~~~~~~~~~~~~~~~

    pub fn read_timeout(&mut self, buffer: &mut [u8], timeout: Option<Duration>) -> Result<usize, TcpError> {
        if self.manager.cancelled() {
            return Err(TcpError::Cancelled);
        }

        let timeout = Timeout::start(timeout);

        match Self::event_read(&mut self.stream, buffer) {
            Ok(Some(len)) => return Ok(len),
            Ok(_) => (),
            Err(error) => return Err(error.into()),
        }

        let mut context = self.manager.context_mut();

        loop {
            let remaining = timeout.remaining_time();
            match context.poll(remaining) {
                Ok(events) => {
                    for _event in events.iter().filter(|event| (event.token() == self.token) && event.is_readable()) {
                        match Self::event_read(&mut self.stream, buffer) {
                            Ok(Some(len)) => return Ok(len),
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

    pub fn read_all_timeout<F>(&mut self, buffer: &mut Vec<u8>, timeout: Option<Duration>, chunk_size: Option<NonZeroUsize>, fn_complete: F) -> Result<(), TcpError>
    where
        F: Fn(&[u8]) -> bool,
    {
        let timeout = Timeout::start(timeout);
        let chunk_size = chunk_size.map(NonZeroUsize::get).unwrap_or(4096);
        let mut valid_length = buffer.len();

        loop {
            buffer.resize(compute_capacity(valid_length, chunk_size), 0);
            let done = match self.read_timeout(&mut buffer[valid_length..], timeout.remaining_time()) {
                Ok(0) => Some(Err(TcpError::Incomplete)),
                Ok(count) => {
                    valid_length += count;
                    match fn_complete(&buffer[..valid_length]) {
                        true => Some(Ok(())),
                        false => None,
                    }
                },
                Err(error) => Some(Err(error)),
            };
            if let Some(result) = done {
                buffer.truncate(valid_length);
                return result;
            }
        }
    }

    fn event_read(stream: &mut MioTcpStream, buffer: &mut [u8]) -> IoResult<Option<usize>> {
        loop {
            match stream.read(buffer) {
                Ok(count) => return Ok(Some(count)),
                Err(error) => match error.kind() {
                    ErrorKind::Interrupted => (),
                    ErrorKind::WouldBlock => return Ok(None),
                    _ => return Err(error),
                },
            }
        }
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~
    // Write functions
    // ~~~~~~~~~~~~~~~~~~~~~~~

    pub fn write_timeout(&mut self, buffer: &[u8], timeout: Option<Duration>) -> Result<usize, TcpError> {
        if self.manager.cancelled() {
            return Err(TcpError::Cancelled);
        }

        let timeout = Timeout::start(timeout);

        match Self::event_write(&mut self.stream, buffer) {
            Ok(Some(len)) => return Ok(len),
            Ok(_) => (),
            Err(error) => return Err(error.into()),
        }

        let mut context = self.manager.context_mut();

        loop {
            let remaining = timeout.remaining_time();
            match context.poll(remaining) {
                Ok(events) => {
                    for _event in events.iter().filter(|event| (event.token() == self.token) && event.is_writable()) {
                        match Self::event_write(&mut self.stream, buffer) {
                            Ok(Some(len)) => return Ok(len),
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

    pub fn write_all_timeout(&mut self, mut buffer: &[u8], timeout: Option<Duration>) -> Result<(), TcpError> {
        let timeout = Timeout::start(timeout);

        loop {
            match self.write_timeout(buffer, timeout.remaining_time()) {
                Ok(0) => return Err(TcpError::Incomplete),
                Ok(count) => {
                    buffer = &buffer[count..];
                    if buffer.is_empty() { return Ok(()); }
                },
                Err(error) => return Err(error),
            };
        }
    }

    fn event_write(stream: &mut MioTcpStream, buffer: &[u8]) -> IoResult<Option<usize>> {
        loop {
            match stream.write(buffer) {
                Ok(count) => return Ok(Some(count)),
                Err(error) => match error.kind() {
                    ErrorKind::Interrupted => (),
                    ErrorKind::WouldBlock => return Ok(None),
                    _ => return Err(error),
                },
            }
        }
    }
}

impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        into_io_result(self.read_timeout(buf, self.timeouts.0))
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        into_io_result(self.write_timeout(buf, self.timeouts.1))
    }

    fn flush(&mut self) -> IoResult<()> {
        self.stream.flush()
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let context = self.manager.context();
        Self::deregister(&context, &mut self.stream);
    }
}

fn into_io_result<T>(result: Result<T, TcpError>) -> IoResult<T> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => Err(error.into()),
    }
}

fn compute_capacity(length: usize, chunk_size: usize) -> usize {
    let padded_len = match length % chunk_size {
        0 => length,
        r => length.checked_add(chunk_size - r).expect("Numerical overflow!"),
    };
    padded_len.checked_add(chunk_size).expect("Numerical overflow!")
}
