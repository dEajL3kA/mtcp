/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
mod flag;
mod timeout;

pub(crate) use flag::Flag;
pub(crate) use timeout::Timeout;
