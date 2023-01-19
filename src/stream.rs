/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::io::{Read, Write, Result, ErrorKind};
use std::net::{SocketAddr, Shutdown};
use std::num::NonZeroUsize;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::time::{Duration};

use mio::{Token, Interest};
use mio::net::TcpStream as MioTcpStream;

use log::warn;

use crate::utilities::{compute_remaining_time, set_up_timeout};
use crate::{TcpConnection, TcpManager};
use crate::error::{ERROR_CANCELLED, ERROR_TIMEDOUT, ERROR_INCOMPLETE};
use crate::manager::TcpPollContext;

/// A TCP stream between a local and a remote socket, akin to
/// [`std::net::TcpStream`](std::net::TcpStream)
///
/// All I/O operations provided by `mtcp_rs::TcpStream` are "blocking" by
/// default, but – unlike the `std::net` implementation – proper ***timeout***
/// and ***cancellation*** support is available. Each `mtcp_rs::TcpStream` is
/// tied to an [`mtcp_rs::TcpManager`](crate::TcpManager) instance.
/// 
/// A new TCP stream is created by [`connect()`](TcpStream::connect())ing to a
/// remote host, or initialized [`from()`](TcpStream::from()) an existing
/// [`mtcp_rs::TcpConnection`](crate::TcpConnection).
/// 
/// If the `timeout` parameter was set to `Some(Duration)` and if the I/O
/// operation does **not** complete before the specified timeout period
/// expires, then the pending I/O operation will fail as soon as possible with
/// an [`TcpError::TimedOut`](crate::TcpError::TimedOut) error.
/// 
/// Function like [`Read::read()`](std::io::Read::read()) and
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
    pub fn from(manager: &Rc<TcpManager>, connection: TcpConnection) -> Result<Self> {
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

    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        self.stream.shutdown(how)
    }

    fn register<T>(context: &T, stream: &mut MioTcpStream) -> Result<Token>
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

    pub fn connect(manager: &Rc<TcpManager>, addr: SocketAddr, timeout: Option<Duration>) -> Result<Self> {
        if manager.cancelled() {
            return ERROR_CANCELLED.result();
        }

        let manager = manager.clone();
        let mut stream = MioTcpStream::connect(addr)?;
        let token;

        {
            let mut context = manager.context_mut();
            token = Self::register(&context, &mut stream)?;

            if let Err(error) = Self::await_connected(&manager, &mut context, &mut stream, token, timeout) {
                Self::deregister(&context, &mut stream);
                return Err(error);
            }    
        }

        Ok(Self {
            stream,
            token,
            timeouts: (None, None),
            manager,
        })
    }

    fn await_connected<T>(manager: &Rc<TcpManager>, context: &mut T, stream: &mut MioTcpStream, token: Token, timeout: Option<Duration>) -> Result<()>
    where
        T: DerefMut<Target=TcpPollContext>
    {
        let timeout = set_up_timeout(timeout);

        loop {
            let remaining = timeout.map(compute_remaining_time);
            match context.poll(remaining) {
                Ok(events) => {
                    for _event in events.iter().filter(|event| (event.token() == token)) {
                        match Self::event_conn(stream) {
                            Ok(true) => return Ok(()),
                            Ok(_) => (),
                            Err(error) => return Err(error),
                        }
                    }
                },
                Err(error) => return Err(error),
            }
            if manager.cancelled() {
                return ERROR_CANCELLED.result();
            }
            if remaining.map(|time| time.is_zero()).unwrap_or(false) {
                return ERROR_TIMEDOUT.result();
            }
        }
    }

    fn event_conn(stream: &mut MioTcpStream) -> Result<bool> {
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

    pub fn read_timeout(&mut self, buffer: &mut [u8], timeout: Option<Duration>) -> Result<usize> {
        if self.manager.cancelled() {
            return ERROR_CANCELLED.result();
        }

        let timeout = set_up_timeout(timeout);

        match Self::event_read(&mut self.stream, buffer) {
            Ok(Some(len)) => return Ok(len),
            Ok(_) => (),
            Err(error) => return Err(error),
        }

        let mut context = self.manager.context_mut();

        loop {
            let remaining = timeout.map(compute_remaining_time);
            match context.poll(remaining) {
                Ok(events) => {
                    for _event in events.iter().filter(|event| (event.token() == self.token) && event.is_readable()) {
                        match Self::event_read(&mut self.stream, buffer) {
                            Ok(Some(len)) => return Ok(len),
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

    pub fn read_all_timeout<F>(&mut self, buffer: &mut Vec<u8>, timeout: Option<Duration>, chunk_size: Option<NonZeroUsize>, fn_complete: F) -> Result<()>
    where
        F: Fn(&[u8]) -> bool,
    {
        let timeout = set_up_timeout(timeout);
        let chunk_size = chunk_size.map(NonZeroUsize::get).unwrap_or(4096);
        let mut valid_length = buffer.len();

        loop {
            buffer.resize(compute_capacity(valid_length, chunk_size), 0);
            let done = match self.read_timeout(&mut buffer[valid_length..], timeout.map(compute_remaining_time)) {
                Ok(0) => Some(ERROR_INCOMPLETE.result()),
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

    fn event_read(stream: &mut MioTcpStream, buffer: &mut [u8]) -> Result<Option<usize>> {
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

    pub fn write_timeout(&mut self, buffer: &[u8], timeout: Option<Duration>) -> Result<usize> {
        if self.manager.cancelled() {
            return ERROR_CANCELLED.result();
        }

        let timeout = set_up_timeout(timeout);

        match Self::event_write(&mut self.stream, buffer) {
            Ok(Some(len)) => return Ok(len),
            Ok(_) => (),
            Err(error) => return Err(error),
        }

        let mut context = self.manager.context_mut();

        loop {
            let remaining = timeout.map(compute_remaining_time);
            match context.poll(remaining) {
                Ok(events) => {
                    for _event in events.iter().filter(|event| (event.token() == self.token) && event.is_writable()) {
                        match Self::event_write(&mut self.stream, buffer) {
                            Ok(Some(len)) => return Ok(len),
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

    pub fn write_all_timeout(&mut self, mut buffer: &[u8], timeout: Option<Duration>) -> Result<()> {
        let timeout = set_up_timeout(timeout);

        loop {
            match self.write_timeout(&buffer[..], timeout.map(compute_remaining_time)) {
                Ok(0) => return ERROR_INCOMPLETE.result(),
                Ok(count) => {
                    buffer = &buffer[count..];
                    if buffer.is_empty() { return Ok(()); }
                },
                Err(error) => return Err(error),
            };
        }
    }

    fn event_write(stream: &mut MioTcpStream, buffer: &[u8]) -> Result<Option<usize>> {
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
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.read_timeout(buf, self.timeouts.0)
    }
}

impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.write_timeout(buf, self.timeouts.1)
    }

    fn flush(&mut self) -> Result<()> {
        self.stream.flush()
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        let context = self.manager.context();
        Self::deregister(&context, &mut self.stream);
    }
}

fn compute_capacity(length: usize, chunk_size: usize) -> usize {
    let padded_len = match length % chunk_size {
        0 => length,
        r => length.checked_add(chunk_size - r).expect("Numerical overflow!"),
    };
    padded_len.checked_add(chunk_size).expect("Numerical overflow!")
}
