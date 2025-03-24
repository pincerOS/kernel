use crate::{Error, Result};

// A ring buffer (bounded buffer) of type `T`.
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
    // Dequeues an element from the head of the buffer, applies the function `f` on it,
    // and returns the result. Returns an error if the buffer is empty.
    //
    // # Returns
    // An error or the result of `f`.
    pub fn dequeue_with<'a, F, R>(&'a mut self, f: F) -> Result<R>
    where
        F: FnOnce(&'a mut T) -> R,
    {
        self.dequeue_maybe(|x| Ok(f(x)))
    }

    // Similar to `dequeue_with` but will cancel the dequeue operation if `f` returns an error.
    //
    // # Returns
    // An error or the result of `f`.
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

    // Enqueues an element at the head of the buffer, applies the function `f` to it,
    // and returns the result. Returns an error if the buffer is full.
    //
    // # Returns
    // An error or the result of `f`.
    pub fn enqueue_with<'a, F, R>(&'a mut self, f: F) -> Result<R>
    where
        F: FnOnce(&'a mut T) -> R,
    {
        self.enqueue_maybe(|x| Ok(f(x)))
    }

    // Similar to `enqueue_with` but will cancel the enqueue operation if `f` returns an error.
    //
    // # Returns
    // An error or the result of `f`.
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

    // Returns the current number of elements in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }
}
