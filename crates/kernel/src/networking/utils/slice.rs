use core::ops::{Deref, DerefMut};
use alloc::vec::Vec;

use crate::networking::{Error, Result};

// dynamically resizable slice that acts as a view over a non-resizable buffer
#[derive(Clone, Debug)]
pub struct Slice<T> {
    buffer: Vec<T>,
    len: usize,
}

impl<T> From<Vec<T>> for Slice<T> {
    fn from(buffer: Vec<T>) -> Self {
        let len = buffer.len();
        Slice { buffer, len }
    }
}

impl<T> Deref for Slice<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.buffer[0..self.len]
    }
}

impl<T> DerefMut for Slice<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.buffer[0..self.len]
    }
}

impl<T: Clone> Slice<T> {
    // attempts to resize the slice, filling the new space with `value` if upsizing.
    pub fn try_resize(&mut self, buffer_len: usize, value: T) -> Result<()> {
        if buffer_len > self.buffer.len() {
            Err(Error::Exhausted)
        } else {
            for i in self.len..buffer_len {
                self.buffer[i] = value.clone();
            }
            self.len = buffer_len;
            Ok(())
        }
    }
}
