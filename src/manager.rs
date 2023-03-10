/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::cell::{RefCell, Ref, RefMut};
use std::io::{Result, ErrorKind};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use mio::{Poll, Events, Token, Waker, Registry};

use lazy_rc::{LazyRc, LazyArc};

use crate::TcpCanceller;
use crate::utilities::Flag;

const SHUTDOWN: Token = Token(usize::MAX);

thread_local! { 
    static INSTANCE: LazyRc<TcpManager> = LazyRc::empty();
}

/// A manager for "shared" resources, used by
/// [`mtcp_rs::TcpListener`](crate::TcpListener) and
/// [`mtcp_rs::TcpStream`](crate::TcpStream)
/// 
/// The *same* `mtcp_rs::TcpManager` instance can be shared by *multiple*
/// `mtcp_rs::TcpListener` and `mtcp_rs::TcpStream` instances. However, an
/// `mtcp_rs::TcpManager` instance can **not** be shared across the thread
/// boundary: Each thread needs to create its own `mtcp_rs::TcpManager` instance.
/// A *thread-local* singleton instance can be obtained via the
/// [`instance()`](TcpManager::instance()) function.
/// 
/// The [`canceller()`](TcpManager::canceller()) function can be used to obtain
/// a new [`mtcp_rs::TcpCanceller`](crate::TcpCanceller) instance for *this*
/// [`mtcp_rs::TcpManager`](crate::TcpCanceller).
#[derive(Debug)]
pub struct TcpManager {
    context: RefCell<TcpPollContext>,
    cancelled: LazyArc<Flag>,
}

#[derive(Debug)]
pub(crate) struct TcpPollContext {
    poll: Poll,
    events: Events,
    next: AtomicUsize,
}

impl TcpManager {
    /// Get the [thread-local](std::thread_local) *singleton* `TcpManager`
    /// instance for the calling thread. The instance is created lazily for
    /// each thread.
    pub fn instance() -> Result<Rc<Self>> {
        INSTANCE.with(|val| val.or_try_init_with(Self::new))
    }

    /// Create a new `TcpManager` instance with *default* queue capacity.
    pub fn new() -> Result<Self> {
        Self::with_capacity(128)
    }

    /// Create a new `TcpManager` instance with the specified queue capacity.
    pub fn with_capacity(capacity: usize) -> Result<Self> {
        let context = TcpPollContext::new(capacity)?;
        Ok(Self {
            context: RefCell::new(context),
            cancelled: LazyArc::empty(),
        })
    }

    /// Create a new [`mtcp_rs::TcpCanceller`](crate::TcpCanceller) instance
    /// for this `TcpManager`.
    pub fn canceller(&self) -> Result<TcpCanceller> {
        self.cancelled
            .or_try_init_with(|| Ok(Flag::new(Waker::new(self.context().poll.registry(), SHUTDOWN)?)))
            .map(TcpCanceller::from)
    }

    /// Check whether this `TcpManager` instance has been
    /// [cancelled](crate::TcpCanceller::cancel) yet.
    /// 
    /// Returns `true`, if cancellation has been requested for this
    /// `TcpManager`; or `false` otherwise.
    pub fn cancelled(&self) -> bool {
        self.cancelled.map(|flag| flag.check()).unwrap_or(false)
    }

    /// Restart the `TcpManager`, i.e. clear its "cancellation" status.
    /// 
    /// Returns `true`, if the `TcpManager` was restarted successfully; or
    /// `false`, if the `TcpManager` was **not** in a "cancelled" state.
    pub fn restart(&self) -> Result<bool> {
        self.cancelled.map(|flag| flag.clear()).unwrap_or(Ok(false))
    }

    pub(crate) fn context(&self) -> Ref<TcpPollContext> {
        self.context.borrow()
    }

    pub(crate) fn context_mut(&self) -> RefMut<TcpPollContext> {
        self.context.borrow_mut()
    }
}

impl TcpPollContext {
    fn new(capacity: usize) -> Result<Self> {
        Ok(Self {
            poll: Poll::new()?,
            events: Events::with_capacity(capacity),
            next: AtomicUsize::new(usize::MIN),
        })
    }

    pub fn token(&self) -> Token {
        loop {
            let token = Token(self.next.fetch_add(1, Ordering::Relaxed));
            if token != SHUTDOWN {
                return token;
            }
        }
    }

    pub fn poll(&mut self, timeout: Option<Duration>) -> Result<&Events>{
        loop {
            match self.poll.poll(&mut self.events, timeout) {
                Ok(_) => return Ok(&self.events),
                Err(error) => {
                    if error.kind() != ErrorKind::Interrupted {
                        return Err(error);
                    }
                },
            }
        }
    }

    pub fn registry(&self) -> &Registry {
        self.poll.registry()
    }
}
