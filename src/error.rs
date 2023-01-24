/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::error::Error;
use std::fmt::{Display, Debug, Formatter};
use std::io::{ErrorKind, Error as IoError, Result as IoResult};

pub(crate) const ERROR_CANCELLED:  ConstError = ConstError::of(ErrorKind::Other,         TcpError::Cancelled);
pub(crate) const ERROR_TIMEDOUT:   ConstError = ConstError::of(ErrorKind::TimedOut,      TcpError::TimedOut);
pub(crate) const ERROR_INCOMPLETE: ConstError = ConstError::of(ErrorKind::UnexpectedEof, TcpError::Incomplete);

/// The error type for **mtcp** I/O operations
/// 
/// **mtcp** I/O operations return a [`std::io::Result`](std::io::Result),
/// which will contain an [`std::io::Error`](std::io::Error) in case of
/// failure. For ***mtcp**-specific* errors, the returned `std::io::Error`
/// contains the suitable variant of `mtcp_rs::TcpError` as its "inner" error.
///
/// Errors from the **`mio`** layer are passed through "as-is"; do **not**
/// expect that an "inner" `mtcp_rs::TcpError` is always available!

#[derive(Copy, Clone)]
pub enum TcpError {
    /// Indicates that the socket operation was *cancelled* before completion.
    /// Data may have been read or written partially!  
    /// The [`kind()`](std::io::Error::kind()) of this error
    /// is:&ensp;**`ErrorKind::Other`**
    Cancelled,
    /// Indicates that the socket operation encountered a time-out. Data may
    /// have been read or written partially!  
    /// The [`kind()`](std::io::Error::kind()) of this error
    /// is:&ensp;**`ErrorKind::TimedOut`**
    TimedOut,
    /// Indicates that the socket operation finished (usually because the
    /// stream was closed) before all data could be read or written.  
    /// The [`kind()`](std::io::Error::kind()) of this error
    /// is:&ensp;**`ErrorKind::UnexpectedEof`**
    Incomplete,
}

pub(crate) struct ConstError {
    kind: ErrorKind,
    tcp_error: TcpError,
}

impl ConstError {
    const fn of(kind: ErrorKind, tcp_error: TcpError) -> Self {
        Self {
            kind,
            tcp_error,
        }
    }
    
    pub fn error(&self) -> IoError {
        IoError::new(self.kind, self.tcp_error)
    }

    pub fn result<T>(&self) -> IoResult<T> {
        Err(self.error())
    }
}

impl Debug for TcpError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "TcpError::Cancelled"),
            Self::TimedOut => write!(f, "TcpError::TimedOut"),
            Self::Incomplete => write!(f, "TcpError::Incomplete"),
        }
    }
}

impl Display for TcpError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TcpError::Cancelled => write!(f, "The TCP socket operation was cancelled!"),
            TcpError::TimedOut => write!(f, "The TCP socket operation timed out!"),
            TcpError::Incomplete => write!(f, "The TCP socket operation is incomplete!"),
        }
    }
}

impl Error for TcpError { }
