/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::io::Result;
use std::sync::Arc;

use crate::utilities::Flag;

/// A canceller that can be used to abort "pending" I/O operations
/// 
/// Each `mtcp_rs::TcpCanceller` instance is tied to an
/// [`mtcp_rs::TcpManager`](crate::TcpManager) instance. Calling the
/// [`cancel()`](TcpCanceller::cancel()) function will *immediately* abort
/// ***any*** pending I/O operations in ***all***
/// [`mtcp_rs::TcpListener`](crate::TcpListener) or
/// [`mtcp_rs::TcpStream`](crate::TcpStream) instances that are tied to the same
/// `mtcp_rs::TcpManager` instance. Unlike the `mtcp_rs::TcpManager` instance, the
/// `mtcp_rs::TcpCanceller` instance *can* be moved across the thread boundary.
/// This is useful, for example, to implement a Ctrl+C (SIGINT) handler.
/// 
/// Cancelled I/O operations will fail with an
/// [`TcpError::Cancelled`](crate::TcpError::Cancelled) error. However, there
/// is **no** guarantee that I/O operations already in progress will actually
/// be cancelled! Even after cancellation has been requested, an I/O operation
/// that was just about to finish may still succeed, or even fail with a
/// different error. Newly started operations *are* guaranteed to be cancelled.
#[derive(Debug)]
pub struct TcpCanceller {
    flag: Arc<Flag>,
}

impl TcpCanceller {
    pub(crate) fn from(flag: Arc<Flag>) -> Self {
        Self {
            flag,
        }
    }

    pub fn cancel(&self) -> Result<bool> {
        self.flag.raise()
    }

    pub fn cancelled(&self) -> bool {
        self.flag.check()
    }
}
