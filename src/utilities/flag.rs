/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

#[derive(Debug)]
pub(crate) struct Flag {
    shared_flag: Arc<AtomicBool>,
}

impl Flag {
    pub fn new() -> Self {
        Self {
            shared_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn raise(&self) -> bool {
        self.shared_flag.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).is_ok()
    }

    pub fn clear(&self) -> bool {
        self.shared_flag.compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed).is_ok()
    }

    pub fn check(&self) -> bool {
        self.shared_flag.load(Ordering::Relaxed)
    }
}

impl Clone for Flag {
    fn clone(&self) -> Self {
        Self {
            shared_flag: self.shared_flag.clone()
        }
    }
}
