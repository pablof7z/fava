//! Bounded command channels.
//!
//! An owner's mailbox has a declared depth. A full mailbox refuses and hands
//! the item back; it never parks the caller and never discards the work
//! silently.
//!
//! Authority: ARCH:2356, GOALS:1448.

use std::num::NonZeroUsize;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::mpsc;

/// Bounded sender.
pub struct Sender<T> {
    sender: mpsc::Sender<T>,
    depth: usize,
}

/// Bounded receiver.
pub struct Receiver<T> {
    receiver: mpsc::Receiver<T>,
    depth: usize,
}

/// Build one bounded channel pair. `closed` creates it already refusing, which
/// is how a runtime that is shutting down admits no new command traffic.
pub(crate) fn build<T>(depth: NonZeroUsize, closed: bool) -> (Sender<T>, Receiver<T>) {
    let depth = depth.get();
    let (sender, mut receiver) = mpsc::channel(depth);
    if closed {
        receiver.close();
    }
    (Sender { sender, depth }, Receiver { receiver, depth })
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            depth: self.depth,
        }
    }
}

impl<T> Sender<T> {
    /// Declared depth of this channel.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Whether every receiver is gone.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }

    /// Enqueue without waiting. A full channel refuses; it never parks.
    ///
    /// # Errors
    ///
    /// [`SendRefused`] when the channel is full or closed. The item is returned
    /// inside the refusal so the caller can report exact shortfall about the
    /// specific command it could not enqueue.
    pub fn try_send(&self, value: T) -> Result<(), SendRefused<T>> {
        match self.sender.try_send(value) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(value)) => Err(SendRefused {
                value,
                reason: SendRefusal::Full { depth: self.depth },
            }),
            Err(mpsc::error::TrySendError::Closed(value)) => Err(SendRefused {
                value,
                reason: SendRefusal::Closed,
            }),
        }
    }

    /// Enqueue, waiting for capacity up to `deadline`.
    ///
    /// # Errors
    ///
    /// [`SendRefused`] on deadline expiry or closure.
    pub async fn send_before(&self, value: T, deadline: Duration) -> Result<(), SendRefused<T>> {
        match tokio::time::timeout(deadline, self.sender.reserve()).await {
            Ok(Ok(permit)) => {
                permit.send(value);
                Ok(())
            }
            Ok(Err(_closed)) => Err(SendRefused {
                value,
                reason: SendRefusal::Closed,
            }),
            Err(_elapsed) => Err(SendRefused {
                value,
                reason: SendRefusal::DeadlineExpired,
            }),
        }
    }

    /// Current queued item count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sender
            .max_capacity()
            .saturating_sub(self.sender.capacity())
    }

    /// Whether the channel is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Receiver<T> {
    /// Declared depth of this channel.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Await the next item; `None` once every sender is dropped.
    pub async fn recv(&mut self) -> Option<T> {
        self.receiver.recv().await
    }

    /// Take an item without waiting.
    #[must_use]
    pub fn try_recv(&mut self) -> Option<T> {
        self.receiver.try_recv().ok()
    }

    /// Refuse further commands while leaving queued work readable.
    pub fn close(&mut self) {
        self.receiver.close();
    }
}

/// A bounded channel refused an item. The item is returned, never dropped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendRefused<T> {
    /// The item that was not enqueued.
    pub value: T,
    /// Why.
    pub reason: SendRefusal,
}

/// Why a bounded channel refused.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SendRefusal {
    /// The channel is at its declared depth.
    #[error("channel is full at its declared depth {depth}")]
    Full {
        /// Declared depth.
        depth: usize,
    },
    /// Every receiver is gone.
    #[error("channel is closed")]
    Closed,
    /// The send deadline expired before capacity appeared.
    #[error("channel send deadline expired")]
    DeadlineExpired,
}
