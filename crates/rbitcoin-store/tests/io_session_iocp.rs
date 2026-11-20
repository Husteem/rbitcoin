#![cfg(windows)]
use rbitcoin_store::io_handle::IoHandle;
use rbitcoin_store::io_session_iocp::*;
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;

const FILE_FLAG_OVERLAPPED: u32 = 0x4000_0000;

fn tmp(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "rbitcoin-iocp-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ))
}

fn open_overlapped(path: &Path) -> std::fs::File {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .custom_flags(FILE_FLAG_OVERLAPPED)
        .open(path)
        .expect("overlapped tmp")
}

/// Closing an overlapped file leaves its HANDLE value in the bind set.
/// The next open often gets that value. The new file is a different object
/// and must be bound again, or its read never posts a completion.
#[test]
fn recycled_handle_pread_completes() {
    // The first close is taken by the next CreateFile's directory open, so
    // that file handle is a new value. Closing that file and opening once
    // more returns the file handle: the directory open consumes the lower
    // free value, and the file gets the one we bound.
    let path_a = tmp("recycle-a");
    let a = open_overlapped(&path_a);
    drop(a);
    let _ = std::fs::remove_file(&path_a);

    let path_b = tmp("recycle-b");
    let b = open_overlapped(&path_b);
    let raw = IoHandle::from_file(&b).as_raw_handle();
    let mut eng = IocpEngine::open(8).unwrap();
    eng.associate(IoHandle::from_file(&b)).unwrap();
    drop(b);
    let _ = std::fs::remove_file(&path_b);

    let path_c = tmp("recycle-c");
    let c = open_overlapped(&path_c);
    let h = IoHandle::from_file(&c);
    assert_eq!(
        h.as_raw_handle(),
        raw,
        "file HANDLE was not reused (got {:?}, bound {raw:?})",
        h.as_raw_handle()
    );
    assert!(h.pwrite(0, b"hello") > 0);
    let mut buf = [0u8; 5];
    eng.push_pread(h, 0, &mut buf, 7).unwrap();
    assert!(
        eng.wait_one_cqe_timeout(2000),
        "recycled HANDLE was not rebound; the completion never arrived"
    );
    assert_eq!(eng.harvest_ready(), vec![(7, 5)]);
    assert_eq!(&buf, b"hello");
    drop(c);
    let _ = std::fs::remove_file(&path_c);
}

#[test]
fn associate_twice_same_port_ok() {
    let path = tmp("twice");
    let f = open_overlapped(&path);
    let h = IoHandle::from_file(&f);
    assert!(h.pwrite(0, b"ab") > 0);
    let mut eng = IocpEngine::open(8).unwrap();
    eng.associate(h).unwrap();
    eng.associate(h).unwrap();
    drop(f);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn associate_non_overlapped_is_error() {
    let path = tmp("sync");
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .expect("sync tmp");
    let h = IoHandle::from_file(&f);
    let mut eng = IocpEngine::open(8).unwrap();
    let err = eng.associate(h).unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("FILE_FLAG_OVERLAPPED"),
        "expected overlapped hint, got {s}"
    );
    drop(f);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn positional_pread_on_associated_handle_skips_completion_port() {
    let path = tmp("xfer-skip");
    let f = open_overlapped(&path);
    let h = IoHandle::from_file(&f);
    assert!(h.pwrite(0, b"hello") > 0);
    let mut eng = IocpEngine::open(8).unwrap();
    eng.associate(h).unwrap();
    let mut buf = [0u8; 5];
    assert_eq!(h.pread(0, &mut buf), 5);
    assert_eq!(&buf, b"hello");
    let extra = eng.harvest_ready();
    assert!(
        extra.is_empty(),
        "positional xfer must not post OVERLAPPED to IOCP (harvest would Box::from_raw a stack frame): {extra:?}"
    );
    drop(f);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn two_threads_bulk_pread_same_handle_completes() {
    use rbitcoin_store::bulk_io::{pread_batch, ReadOp};
    use std::sync::atomic::{AtomicU32, Ordering};

    let path = tmp("two-thread-pread");
    let f = open_overlapped(&path);
    let h = IoHandle::from_file(&f);
    assert!(h.pwrite(0, b"hello") > 0);
    let done = AtomicU32::new(0);
    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                let mut buf = [0u8; 5];
                let mut ops = [ReadOp {
                    fd: h,
                    offset: 0,
                    buf: &mut buf,
                    result: i32::MIN,
                }];
                pread_batch(&mut ops);
                assert!(
                    ops[0].result >= 5,
                    "bulk pread must complete (not drain-hang) result={}",
                    ops[0].result
                );
                assert_eq!(&buf, b"hello");
                done.fetch_add(1, Ordering::Relaxed);
            });
        }
    });
    assert_eq!(done.load(Ordering::Relaxed), 2);
    drop(f);
    let _ = std::fs::remove_file(&path);
}
