/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */

//! **mtcp** provides a "blocking" implementation of `TcpListener` and
//! `TcpStream` with proper ***timeout*** and ***cancellation*** support.
//! 
//! [`mtcp_rs::TcpListener`](TcpListener) and [`mtcp_rs::TcpStream`](TcpStream)
//! pretty much are drop-in replacements for
//! [`std::net::TcpListener`](std::net::TcpListener),
//! [`std::net::TcpStream`](std::net::TcpListener), but with an additional
//! *timeout* parameter in the "blocking" I/O functions – including but not
//! limited to the `accept()` function! Also, a
//! [`mtcp_rs::TcpCanceller`](TcpCanceller) can be used to abort "pending" I/O
//! operation immediately, e.g. from another thread or from the Ctrl+C (SIGINT)
//! handler, so that "cleanly" shutting down your server becomes a possibility.
//! 
//! The "blocking" I/O operations in **mtcp** are emulated via *non-blocking*
//! operations, using the [**`mio`**](mio) library, in order to make timeouts
//! and cancellation support possible while also providing very high
//! performance. But, thanks to **mtcp**, you won't have to bother  with `mio`
//! events and the event polling mechanism at all. All platforms supported by
//! `mio` are supported by **mtcp** as well.
//! 
//! # Usage
//! 
//! First of all, a [`mtcp_rs::TcpManager`](TcpManager) instance for the
//! current thread must be obtained. Then a new
//! [`mtcp_rs::TcpListener`](TcpListener) instance can be bound to a local
//! socket. New incoming connections are returned in the form of
//! [`mtcp_rs::TcpConnection`](TcpConnection) instances. Usually an
//! [`mtcp_rs::TcpConnection`](TcpConnection) instance is converted into an
//! [`mtcp_rs::TcpStream`](TcpStream) for read/write access.
//! 
//! The function [`TcpManager::canceller()`](TcpManager) optionally provides a
//! new [`mtcp_rs::TcpCanceller`](TcpCanceller) instance that may be used to
//! ***cancel*** pending I/O operations. You can use, for example,
//! [`ctrlc`](https://crates.io/crates/ctrlc) to invoke
//! [`cancel()`](TcpCanceller::cancel()) from your Ctrl+C (SIGINT) handler.
//! 
//! # Examples
//! 
//! Examples be can found in the `examples` sub-directory, or on
//! [**GitHub**](https://github.com/dEajL3kA/mtcp/tree/master/examples).

mod canceller;
mod connection;
mod stream;
mod manager;
mod error;
mod listener;
mod utilities;

pub use canceller::TcpCanceller;
pub use connection::TcpConnection;
pub use error::TcpError;
pub use listener::TcpListener;
pub use manager::TcpManager;
pub use stream::TcpStream;
