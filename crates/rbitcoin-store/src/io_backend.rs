//! Bulk-IO backend selection for Class A body (no mmap payload).
//!
//! ## Operator env (single switch)
//! - `RBITCOIN_IO=uring|pool|iocp|pread` — all bulk read/write paths
//!   (default: uring if available). Unknown tokens fall through to the default.
//!
//! Per-path env overrides are **removed** — one global `RBITCOIN_IO` only.
//!
//! Class A **tx.body** payload is always pread/pwrite/uring — never mmap.

use crate::bulk_io;
use std::sync::OnceLock;

/// Bulk **read** backend (body denserels, head prefix, meta peeks, Class C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadIoBackend {
    /// io_uring pread batch / streaming.
    Uring,
    /// libc pread (optionally multi-worker via `RBITCOIN_BULK_IO_WORKERS`).
    Pread,
}

#[doc(hidden)]
pub fn parse_read_token(s: &str) -> Option<ReadIoBackend> {
    Some(match crate::bulk_io::parse_io_token_str(s)? {
        crate::bulk_io::IoToken::Uring
        | crate::bulk_io::IoToken::Pool
        | crate::bulk_io::IoToken::Iocp => ReadIoBackend::Uring,
        crate::bulk_io::IoToken::Pread => ReadIoBackend::Pread,
    })
}

#[doc(hidden)]
pub fn global_read_from_env_str(s: &str) -> Option<ReadIoBackend> {
    parse_read_token(s)
}

#[doc(hidden)]
pub fn global_read_from_env() -> Option<ReadIoBackend> {
    global_read_from_vars(|k| std::env::var(k).ok())
}

#[doc(hidden)]
pub fn global_read_from_vars(mut var: impl FnMut(&str) -> Option<String>) -> Option<ReadIoBackend> {
    var("RBITCOIN_IO").as_deref().and_then(parse_read_token)
}

#[inline]
pub fn effective_read(selected: ReadIoBackend) -> ReadIoBackend {
    match selected {
        ReadIoBackend::Uring if !bulk_io::io_uring_enabled() => ReadIoBackend::Pread,
        other => other,
    }
}

#[doc(hidden)]
pub fn default_read() -> ReadIoBackend {
    if bulk_io::io_uring_enabled() {
        ReadIoBackend::Uring
    } else {
        ReadIoBackend::Pread
    }
}

/// Global bulk read backend (`RBITCOIN_IO` only — no path overrides).
pub fn resolve_read() -> ReadIoBackend {
    let selected = global_read_from_env().unwrap_or_else(default_read);
    effective_read(selected)
}

#[doc(hidden)]
pub fn global_read_cached() -> ReadIoBackend {
    static B: OnceLock<ReadIoBackend> = OnceLock::new();
    *B.get_or_init(resolve_read)
}

/// Bulk read backend for every path (pin, head-resolve, spend-meta, Class C).
#[inline]
pub fn read_io_backend() -> ReadIoBackend {
    global_read_cached()
}
