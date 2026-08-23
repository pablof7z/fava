//! Bounded command and completion channels.

use std::marker::PhantomData;

use thiserror::Error;

/// Typed refusal from a bounded channel.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum Backpressure {
    /// The channel already holds its declared capacity.
    #[error("channel is at its declared capacity of {capacity}")]
    Full {
        /// Declared capacity.
        capacity: usize,
    },
    /// The owning receiver is gone.
    #[error("channel receiver is closed")]
    Closed,
}

/// Sending half of a bounded channel.
#[derive(Debug)]
pub struct BoundedSender<T> {
    _placeholder: PhantomData<T>,
}

/// Receiving half of a bounded channel.
#[derive(Debug)]
pub struct BoundedReceiver<T> {
    _placeholder: PhantomData<T>,
}

/// Create a bounded channel with at least one slot.
#[must_use]
pub fn bounded_channel<T>(_capacity: usize) -> (BoundedSender<T>, BoundedReceiver<T>) {
    todo!()
}

impl<T> Clone for BoundedSender<T> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<T> BoundedSender<T> {
    /// Declared capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        todo!()
    }

    /// Whether the owning receiver is gone.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        todo!()
    }

    /// Admit one value, or refuse with a typed backpressure fact.
    pub fn try_send(&self, _value: T) -> Result<(), Backpressure> {
        todo!()
    }
}

impl<T> BoundedReceiver<T> {
    /// Receive the next value, or `None` once every sender is gone.
    pub async fn recv(&mut self) -> Option<T> {
        todo!()
    }

    /// Receive a value already queued.
    pub fn try_recv(&mut self) -> Option<T> {
        todo!()
    }
}
