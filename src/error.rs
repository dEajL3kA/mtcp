/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::error::Error;
use std::fmt::{Display, Debug, Formatter};
use std::io::{ErrorKind, Error as IoError};

/// The error type for **mtcp** I/O operations
/// 
/// **mtcp** I/O operations return a [`std::io::Result`](std::io::Result),
/// which will contain an [`std::io::Error`](std::io::Error) in case of
/// failure. For ***mtcp**-specific* errors, the returned `std::io::Error`
/// contains the suitable variant of `mtcp_rs::TcpError` as its "inner" error.
///
/// Errors from the **`mio`** layer are passed through "as-is"; do **not**
/// expect that an "inner" `mtcp_rs::TcpError` is always available!

pub enum TcpError {
    /// Indicates that the socket operation was *cancelled* before completion.
    /// Data may have been read or written partially!
    Cancelled,
    /// Indicates that the socket operation encountered a time-out. Data may
    /// have been read or written partially!
    TimedOut,
    /// Indicates that the socket operation finished (usually because the
    /// stream was closed) before all data could be read or written.
    Incomplete,
    /// Indicates that the socket *read* operation was aborted, because the
    /// length of the data would have exceeded the specified limit.
    TooBig,
    /// Indicates that the socket operation has failed. More detailed
    /// information is available via the wrapped [`io::Error`](std::io::Error).
    Failed(IoError)
}

impl Error for TcpError { }

impl Debug for TcpError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "TcpError::Cancelled"),
            Self::TimedOut => write!(f, "TcpError::TimedOut"),
            Self::Incomplete => write!(f, "TcpError::Incomplete"),
            Self::TooBig => write!(f, "TcpError::TooBig"),
            Self::Failed(error) => write!(f, "TcpError::Failed({:?})", error),
        }
    }
}

impl Display for TcpError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "The TCP socket operation was cancelled!"),
            Self::TimedOut => write!(f, "The TCP socket operation timed out!"),
            Self::Incomplete => write!(f, "The TCP socket operation is incomplete!"),
            Self::TooBig => write!(f, "The TCP socket operation aborted, data is too big!"),
            Self::Failed(error) => write!(f, "{}", error),
        }
    }
}

impl From<TcpError> for IoError {
    fn from(value: TcpError) -> Self {
        match value {
            TcpError::Failed(error) => error,
            other => IoError::new(ErrorKind::Other, other),
        }
    }
}

impl From<IoError> for TcpError {
    fn from(value: IoError) -> Self {
        match try_downcast::<TcpError>(value) {
            Ok(error) => error,
            Err(other) => TcpError::Failed(other),
        }
    }
}

fn try_downcast<T: Error + 'static>(error: IoError) -> Result<T, IoError> {
    match error.get_ref().map(|inner| inner.is::<T>()) {
        Some(true) => Ok(*error.into_inner().unwrap().downcast::<T>().unwrap()),
        _ => Err(error),
    }
}
