/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::time::{Instant, Duration};

pub struct Timeout {
    timeout: Option<(Instant, Duration)>,
}

impl Timeout {
    pub fn start(timeout: Option<Duration>) -> Self {
        Self {
            timeout: timeout.map(|duration| (Instant::now(), duration))
        }
    }

    pub fn remaining_time(&self) -> Option<Duration> {
        self.timeout.map(|(start, duration)| duration.saturating_sub(start.elapsed()))
    }
}
