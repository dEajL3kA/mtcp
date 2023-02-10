/*
 * mtcp - TcpListener/TcpStream *with* timeout/cancellation support
 * This is free and unencumbered software released into the public domain.
 */
use std::io::{Result as IoResult, Error as IoError, ErrorKind};
use std::num::NonZeroUsize;
use std::slice::from_raw_parts_mut;

pub struct BufferManager<'a> {
    buffer: &'a mut Vec<u8>,
    limit: usize,
}

impl<'a> BufferManager<'a> {
    pub fn from(buffer: &'a mut Vec<u8>, limit: Option<NonZeroUsize>) -> Self {
        Self {
            buffer,
            limit: limit.map_or(usize::MAX, NonZeroUsize::get),
        }
    }

    pub fn valid_data(&self) -> &[u8] {
        &self.buffer[..]
    }

    pub fn alloc_spare_buffer(&mut self, min_length: NonZeroUsize) -> &'a mut[u8] {
        self.buffer.reserve(min_length.get());
        let spare = self.buffer.spare_capacity_mut();
        unsafe {
            from_raw_parts_mut(spare.as_mut_ptr() as *mut u8, spare.len())
        }
    }

    pub fn commit(&mut self, additional: usize) -> IoResult<()> {
        if additional > 0 {
            let new_length = self.buffer.len().checked_add(additional).expect("Numerical overflow!");
            if new_length <= self.limit {
                assert!(new_length <= self.buffer.capacity());
                unsafe {
                    self.buffer.set_len(new_length)
                }
            } else {
                return Err(IoError::new(ErrorKind::OutOfMemory, "New length exceeds the limit!"))
            }
        }
        Ok(())
    }
}
