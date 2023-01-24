/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Debug)]
pub struct LazyCell<T> {
    container: RefCell<Option<Rc<T>>>,
}

impl<T> LazyCell<T> {
    pub fn empty() -> Self {
        Self {
            container: RefCell::new(None),
        }
    }

    pub fn or_init_with<E, FnInit>(&self, init_fn: FnInit) -> Result<Rc<T>, E>
    where
        FnInit: FnOnce() -> Result<Rc<T>, E>
    {
        let mut container = self.container.borrow_mut();
        match container.as_ref() {
            Some(existing) => Ok(existing.clone()),
            None => match init_fn() {
                Ok(value) => Ok(container.insert(value).clone()),
                Err(error) => Err(error),
            }
        }
    }
}
