//! Cancellation signals owned by lifecycle owners and propagated by the runtime.
//!
//! Authority: ARCH:2363.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use tokio::sync::Notify;

/// A cancellation signal owned by a lifecycle owner and propagated by the runtime.
///
/// Cloning yields another handle to the *same* signal. [`CancellationToken::child`]
/// yields a distinct signal that fires when its parent fires and can also fire
/// on its own without disturbing its parent or siblings.
///
/// Every token is rooted in one runtime's shutdown token, so shutdown reaches
/// every owner that derived one.
#[derive(Clone)]
pub struct CancellationToken {
    node: Arc<Node>,
}

struct Node {
    cancelled: AtomicBool,
    notify: Notify,
    children: Mutex<Vec<Weak<Node>>>,
    /// A child keeps its ancestors alive so that propagation never depends on
    /// an owner holding every intermediate handle. Parents refer to children
    /// weakly, so there is no cycle and a dropped subtree is reclaimed.
    ///
    /// The field is never read: holding it is the whole behaviour.
    #[expect(dead_code, reason = "ownership edge that keeps the parent chain alive")]
    parent: Option<Arc<Node>>,
}

impl Node {
    fn fresh(parent: Option<Arc<Self>>) -> Arc<Self> {
        Arc::new(Self {
            cancelled: AtomicBool::new(false),
            notify: Notify::new(),
            children: Mutex::new(Vec::new()),
            parent,
        })
    }

    fn children(&self) -> std::sync::MutexGuard<'_, Vec<Weak<Self>>> {
        self.children.lock().unwrap_or_else(|poison| {
            self.children.clear_poison();
            poison.into_inner()
        })
    }

    /// Fire this node and hand back the live children still to fire.
    fn fire(&self) -> Vec<Arc<Self>> {
        if self.cancelled.swap(true, Ordering::AcqRel) {
            return Vec::new();
        }
        self.notify.notify_waiters();
        std::mem::take(&mut *self.children())
            .iter()
            .filter_map(Weak::upgrade)
            .collect()
    }
}

impl CancellationToken {
    /// The runtime's root token. Private: owners receive tokens from a runtime.
    pub(crate) fn root() -> Self {
        Self {
            node: Node::fresh(None),
        }
    }

    /// A token that fires when this one fires.
    ///
    /// A child derived from an already-fired token is created fired. The parent
    /// retains only weak references and prunes dropped children on every
    /// derivation, so the retained set is bounded by the live children.
    #[must_use]
    pub fn child(&self) -> Self {
        let child = Node::fresh(Some(Arc::clone(&self.node)));
        if self.node.cancelled.load(Ordering::Acquire) {
            child.cancelled.store(true, Ordering::Release);
            return Self { node: child };
        }

        let mut children = self.node.children();
        children.retain(|weak| weak.strong_count() > 0);
        children.push(Arc::downgrade(&child));
        drop(children);

        // The parent may have fired between the check and the push.
        if self.node.cancelled.load(Ordering::Acquire) {
            child.cancelled.store(true, Ordering::Release);
            child.notify.notify_waiters();
        }
        Self { node: child }
    }

    /// Fire this token and every descendant.
    ///
    /// Repeated cancellation is harmless. Propagation is iterative, so a deep
    /// token tree cannot overflow the stack.
    pub fn cancel(&self) {
        let mut pending = self.node.fire();
        while let Some(node) = pending.pop() {
            pending.extend(node.fire());
        }
    }

    /// Whether this token has fired.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.node.cancelled.load(Ordering::Acquire)
    }

    /// Resolve when this token fires.
    pub async fn cancelled(&self) {
        while !self.is_cancelled() {
            let notified = self.node.notify.notified();
            tokio::pin!(notified);
            // Register before re-checking, so a cancellation landing in between
            // still wakes this waiter.
            notified.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}
