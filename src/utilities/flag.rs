/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::io::Result;
use std::sync::atomic::{AtomicBool, Ordering};

use mio::Waker;

#[derive(Debug)]
pub(crate) struct Flag {
    waker: Waker,
    flag: AtomicBool,
}

impl Flag {
    pub fn new(waker: Waker) -> Self {
        Self {
            waker,
            flag: AtomicBool::new(false),
        }
    }

    pub fn raise(&self) -> Result<bool> {
        match self.flag.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => self.waker.wake().map(always),
            Err(_) => Ok(false)
        }
    }

    pub fn clear(&self) -> Result<bool> {
        match self.flag.compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => self.waker.wake().map(always),
            Err(_) => Ok(false)
        }
    }

    pub fn check(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}

pub fn always<T>(_: T) -> bool {
    true
}
