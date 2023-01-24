/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
mod flag;
mod misc;

pub(crate) use flag::Flag;
pub(crate) use misc::{set_up_timeout, compute_remaining_time};
