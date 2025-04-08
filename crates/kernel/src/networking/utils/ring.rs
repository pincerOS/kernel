use crate::networking::{Error, Result};
use alloc::vec::Vec;

#[derive(Clone, Debug)]
pub struct Ring<T> {
    buffer: Vec<T>,
    begin: usize,
    len: usize,
}

impl<T> From<Vec<T>> for Ring<T> {
    fn from(buffer: Vec<T>) -> Self {
        Ring {
            buffer,
            begin: 0,
            len: 0,
        }
    }
}

impl<T> Ring<T> {
    // dequeues an element from the head of the buffer, applies the function f on it,
    // and returns the result. 
    //
    // returns an error if the buffer is empty
    pub fn dequeue_with<'a, F, R>(&'a mut self, f: F) -> Result<R>
    where
        F: FnOnce(&'a mut T) -> R,
    {
        self.dequeue_maybe(|x| Ok(f(x)))
    }

    // dequeue_with but will cancel the dequeue operation if f returns an error.
    pub fn dequeue_maybe<'a, F, R>(&'a mut self, f: F) -> Result<R>
    where
        F: FnOnce(&'a mut T) -> Result<R>,
    {
        if self.len == 0 {
            return Err(Error::Exhausted);
        }

        let buffer_len = self.buffer.len();

        match f(&mut self.buffer[self.begin]) {
            Err(err) => Err(err),
            Ok(res) => {
                self.begin = (self.begin + 1) % buffer_len;
                self.len -= 1;
                Ok(res)
            }
        }
    }

    // enqueues an element at the head of the buffer, applies the function f to it,
    // and returns the result. 
    //
    // returns an error if the buffer is full.
    pub fn enqueue_with<'a, F, R>(&'a mut self, f: F) -> Result<R>
    where
        F: FnOnce(&'a mut T) -> R,
    {
        self.enqueue_maybe(|x| Ok(f(x)))
    }

    // enqueue_with but will cancel the enqueue operation if f returns an error.
    pub fn enqueue_maybe<'a, F, R>(&'a mut self, f: F) -> Result<R>
    where
        F: FnOnce(&'a mut T) -> Result<R>,
    {
        if self.len == self.buffer.len() {
            return Err(Error::Exhausted);
        }

        let idx = (self.begin + self.len) % self.buffer.len();

        match f(&mut self.buffer[idx]) {
            Err(err) => Err(err),
            Ok(res) => {
                self.len += 1;
                Ok(res)
            }
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }
    
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
