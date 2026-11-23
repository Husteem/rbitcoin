//! Response-write deadline for the HTTP listeners (Esplora and RPC/REST).

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Per-connection response counter. The response-start middleware bumps it
/// when a response's headers are ready; the socket wrapper starts that
/// response's byte and stall totals on the next write. Shared by the Esplora and RPC listeners.
#[derive(Default)]
pub struct ResponseBudget {
    generation: AtomicU64,
}

impl ResponseBudget {
    pub fn start(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
}

/// A pending response write is cancelled after `timeout` without progress,
/// and a response whose writes spend more than `timeout + written / min_rate`
/// stalled time is cancelled too. Reads keep their header/idle deadline; this
/// bounds only the write side, so a reader that accepts a few bytes before
/// each no-progress deadline can no longer hold the connection and its accept
/// slot. Server compute between writes is not stalled time. Shared by the Esplora and RPC listeners.
pub struct WriteDeadline<S> {
    inner: S,
    timeout: Duration,
    min_rate: u64,
    budget: Arc<ResponseBudget>,
    response: u64,
    written: u64,
    stalled_for: Duration,
    open_stall: Option<tokio::time::Instant>,
    armed: bool,
    sleep: Pin<Box<tokio::time::Sleep>>,
}

impl<S> WriteDeadline<S> {
    pub fn new(inner: S, timeout: Duration, min_rate: u64, budget: Arc<ResponseBudget>) -> Self {
        Self {
            inner,
            timeout,
            min_rate: min_rate.max(1),
            response: budget.generation.load(Ordering::Relaxed),
            budget,
            written: 0,
            stalled_for: Duration::ZERO,
            open_stall: None,
            armed: false,
            sleep: Box::pin(tokio::time::sleep(timeout)),
        }
    }

    fn progressed(&mut self) {
        self.armed = false;
    }

    /// Start the totals for a response the middleware began, charge the stall
    /// interval that just ended, and reject a response that spent more than
    /// its no-progress grace plus `written / min_rate` stalled.
    fn drain(&mut self) -> Option<io::Error> {
        let generation = self.budget.generation.load(Ordering::Relaxed);
        if generation != self.response {
            self.response = generation;
            self.written = 0;
            self.stalled_for = Duration::ZERO;
            self.open_stall = None;
            self.armed = false;
        }
        if let Some(start) = self.open_stall.take() {
            self.stalled_for += start.elapsed();
        }
        let allowance =
            self.timeout + Duration::from_secs_f64(self.written as f64 / self.min_rate as f64);
        (self.stalled_for > allowance).then(|| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                "http: response drained below the minimum rate",
            )
        })
    }

    fn stalled(&mut self, cx: &mut Context<'_>) -> bool {
        if !self.armed {
            self.sleep
                .as_mut()
                .reset(tokio::time::Instant::now() + self.timeout);
            self.armed = true;
        }
        self.sleep.as_mut().poll(cx).is_ready()
    }

    /// Mark the socket blocked and report whether the no-progress deadline
    /// passed. The open interval is charged by the next [`Self::drain`].
    fn blocked(&mut self, cx: &mut Context<'_>) -> bool {
        self.open_stall = Some(tokio::time::Instant::now());
        self.stalled(cx)
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for WriteDeadline<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for WriteDeadline<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        AsyncWrite::poll_write_vectored(self, cx, &[io::IoSlice::new(buf)])
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if let Some(err) = this.drain() {
            return Poll::Ready(Err(err));
        }
        match Pin::new(&mut this.inner).poll_write_vectored(cx, bufs) {
            Poll::Ready(result) => {
                if let Ok(n) = &result {
                    this.written += *n as u64;
                }
                this.progressed();
                Poll::Ready(result)
            }
            Poll::Pending => {
                if this.blocked(cx) {
                    Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "http: response write stalled",
                    )))
                } else {
                    Poll::Pending
                }
            }
        }
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if let Some(err) = this.drain() {
            return Poll::Ready(Err(err));
        }
        match Pin::new(&mut this.inner).poll_flush(cx) {
            Poll::Ready(result) => {
                this.progressed();
                Poll::Ready(result)
            }
            Poll::Pending => {
                if this.blocked(cx) {
                    Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "http: response flush stalled",
                    )))
                } else {
                    Poll::Pending
                }
            }
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if let Some(err) = this.drain() {
            return Poll::Ready(Err(err));
        }
        match Pin::new(&mut this.inner).poll_shutdown(cx) {
            Poll::Ready(result) => {
                this.progressed();
                Poll::Ready(result)
            }
            Poll::Pending => {
                if this.blocked(cx) {
                    Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "http: response shutdown stalled",
                    )))
                } else {
                    Poll::Pending
                }
            }
        }
    }
}
