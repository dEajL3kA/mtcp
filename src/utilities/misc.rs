use std::time::{Instant, Duration};

pub fn set_up_timeout(timeout: Option<Duration>) -> Option<(Instant, Duration)> {
    match timeout {
        Some(value) => Some((Instant::now(), value)),
        None => None,
    }
}

pub fn compute_remaining_time(timeout: (Instant, Duration)) -> Duration {
    timeout.1.saturating_sub(timeout.0.elapsed())
}
