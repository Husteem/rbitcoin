//! One session's outbound queue and its byte count (Core `nSendSize`).
//!
//! [`PeerOutTx::send`] charges every message it queues, and the charge rides
//! on the [`Queued`] item. Dropping the item, or its [`Charge`] once the
//! writer took it apart, releases exactly that charge, so the count is the
//! sum of the charges still alive. A dropped channel releases whatever was
//! still on it.

use crate::error::NetError;
use crate::peers::{outbound_queued_bytes, PeerOut, PEER_SEND_BUDGET};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Notify};

/// Queued bytes and served bodies on one session's writer channel.
#[derive(Debug, Default)]
pub struct SendQueue {
    bytes: AtomicUsize,
    served: AtomicUsize,
    resume: Notify,
}

impl SendQueue {
    pub fn channel(self: &Arc<Self>) -> (PeerOutTx, PeerOutRx) {
        let (tx, rx) = mpsc::unbounded_channel();
        let tx = PeerOutTx {
            tx,
            queue: Arc::clone(self),
        };
        (tx, PeerOutRx(rx))
    }

    pub fn bytes(&self) -> usize {
        self.bytes.load(Ordering::Relaxed)
    }

    /// Queued items that hold a serve slot ([`PeerOut::holds_serve_slot`]).
    pub fn served(&self) -> usize {
        self.served.load(Ordering::SeqCst)
    }

    /// Notified when a release leaves the queue at or under
    /// [`PEER_SEND_BUDGET`], and on [`Self::wake`].
    pub fn resume(&self) -> &Notify {
        &self.resume
    }

    pub fn wake(&self) {
        self.resume.notify_waiters();
    }
}

/// A writer channel with its own [`SendQueue`], for a session that has no
/// [`crate::peers::LivePeer`]. A live session takes its channel from
/// [`crate::peers::LivePeer::open_out`], whose budget reads that queue.
pub fn peer_out_channel() -> (PeerOutTx, PeerOutRx) {
    Arc::new(SendQueue::default()).channel()
}

/// The only way onto a session's writer channel.
#[derive(Clone)]
pub struct PeerOutTx {
    tx: mpsc::UnboundedSender<Queued>,
    queue: Arc<SendQueue>,
}

impl PeerOutTx {
    pub fn send(&self, out: impl Into<PeerOut>) -> Result<(), NetError> {
        let out = out.into();
        let charge = Charge::new(&self.queue, &out);
        self.tx
            .send(Queued { out, charge })
            .map_err(|_| NetError::Protocol("peer write half closed"))
    }

    /// The writer task dropped its [`PeerOutRx`].
    pub async fn closed(&self) {
        self.tx.closed().await;
    }
}

pub struct PeerOutRx(mpsc::UnboundedReceiver<Queued>);

impl PeerOutRx {
    pub async fn recv(&mut self) -> Option<Queued> {
        self.0.recv().await
    }

    pub fn try_recv(&mut self) -> Result<Queued, mpsc::error::TryRecvError> {
        self.0.try_recv()
    }
}

/// One queued message and the charge taken for it.
#[derive(Debug)]
pub struct Queued {
    out: PeerOut,
    charge: Charge,
}

impl Queued {
    pub fn out(&self) -> &PeerOut {
        &self.out
    }

    pub fn into_parts(self) -> (PeerOut, Charge) {
        (self.out, self.charge)
    }

    #[cfg(debug_assertions)]
    pub fn into_out(self) -> PeerOut {
        self.into_parts().0
    }

    #[cfg(debug_assertions)]
    pub fn expect_msg(self) -> bitcoin::p2p::message::NetworkMessage {
        self.into_out().expect_msg()
    }
}

/// Bytes (and a serve slot) still counted on the queue until this drops.
#[derive(Debug)]
#[must_use = "dropping the charge releases it"]
pub struct Charge {
    queue: Arc<SendQueue>,
    bytes: usize,
    served: bool,
}

impl Charge {
    fn new(queue: &Arc<SendQueue>, out: &PeerOut) -> Self {
        let bytes = outbound_queued_bytes(out);
        let served = out.holds_serve_slot();
        if served {
            queue.served.fetch_add(1, Ordering::SeqCst);
        }
        queue.bytes.fetch_add(bytes, Ordering::Relaxed);
        Self {
            queue: Arc::clone(queue),
            bytes,
            served,
        }
    }
}

impl Drop for Charge {
    fn drop(&mut self) {
        // The slot is freed before the wake: a paused getdata tail woken by
        // the byte release must already see the room.
        if self.served {
            self.queue.served.fetch_sub(1, Ordering::SeqCst);
        }
        let left = self.queue.bytes.fetch_sub(self.bytes, Ordering::Relaxed) - self.bytes;
        if left <= PEER_SEND_BUDGET {
            self.queue.wake();
        }
    }
}
