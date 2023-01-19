/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
mod lazy_cell;
mod flag;
mod misc;

pub(crate) use flag::Flag;
pub(crate) use lazy_cell::LazyCell;
pub(crate) use misc::{set_up_timeout, compute_remaining_time};
