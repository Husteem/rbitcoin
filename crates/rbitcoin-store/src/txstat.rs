//! Dense **create_fk-ordered** confirm-time econ (`txstat.body`).
//!
//! Layout (schema 25):
//! ```text
//! txstat.body  offset 0..32              — TableFile 16 + 16 pad
//!              offset 32+(fk-1)×8        — 8-byte cell
//! txstat.ovf   append-only tails          — remaining ULEB bytes when the
//!                                          four-field stream exceeds 8 B
//! txstat.blk   offset 32+(header_fk-1)×16 — off:u64, len:u32, n_ovf:u32
//! ```
//!
//! Cell payload is three canonical ULEBs: `fee_sat`, `base` (non-witness
//! size), `wit_extra` (`total_size − base`). `n_in` lives on `input.loc`.
//! `size = base + wit_extra`, `weight = 4×base + wit_extra`. All-zero cell =
//! unstamped. A truncated ULEB or fewer than three fields means the rest of
//! the stream is in that header's overflow blob (`encoded[8..]`). Pin / SH /
//! tweaks do not open these files.

use crate::durability::Durable as _;
use crate::error::StoreError;
use crate::file::{self, GrowPolicy, TableFile, FILE_HEADER_LEN};
use bitcoin_hashes::{sha256, Hash, HashEngine};
use rbitcoin_primitives::{
    le_u32, le_u64, read_uleb128, uleb128_len, write_uleb128, Fk, TableKind,
};
use std::collections::BTreeMap;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Bytes before first body cell (TableFile header 16 + pad to 32).
pub const TXSTAT_BODY_HEADER: u64 = 32;
pub const TXSTAT_ENTRY_LEN: u64 = 8;
#[doc(hidden)]
pub const BLK_HEADER: u64 = 32;
#[doc(hidden)]
pub const BLK_SLOT: u64 = 16;
/// Overflow blob ceiling. A longer on-disk length is corrupt, not an allocation.
#[doc(hidden)]
pub const TXSTAT_BLOB_CEILING: usize = 16 * 1024 * 1024;

const AHEAD_MAGIC: &[u8; 4] = b"TTA1";
const AHEAD_VERSION: u16 = 1;
const AHEAD_HEAD: usize = 20;
const AHEAD_REC: usize = 8 + 32 + 4096;

#[cfg(debug_assertions)]
static FAIL_AHEAD: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Leave the next txstat page image behind and fail before the pwrite.
#[cfg(debug_assertions)]
#[doc(hidden)]
pub fn fail_next_txstat_flush_after_ahead(path: &Path) {
    *FAIL_AHEAD.lock().unwrap_or_else(|e| e.into_inner()) = Some(path.to_path_buf());
}

#[cfg(debug_assertions)]
fn take_fail_ahead(path: &Path) -> bool {
    let mut fail = FAIL_AHEAD.lock().unwrap_or_else(|e| e.into_inner());
    if fail.as_deref() == Some(path) {
        *fail = None;
        true
    } else {
        false
    }
}

fn ahead_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("txstat"))
        .to_os_string();
    name.push(".ahead");
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

fn read_page(path: &Path, file: &TableFile, block: u64) -> Result<[u8; 4096], StoreError> {
    let mut image = [0u8; 4096];
    let len = std::fs::metadata(path)
        .map_err(|e| StoreError::io(path, e))?
        .len();
    if block < len {
        let n = ((len - block) as usize).min(4096);
        file.pread_all(block, &mut image[..n])?;
    }
    Ok(image)
}

/// Apply full-page images left by an interrupted `write_at_head`.
fn restore_ahead(path: &Path) -> Result<(), StoreError> {
    let side = ahead_path(path);
    let raw = match std::fs::read(&side) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(StoreError::io(&side, e)),
    };
    if raw.len() < AHEAD_HEAD || raw[0..4] != *AHEAD_MAGIC {
        return Err(StoreError::Corrupt("txstat ahead header"));
    }
    if u16::from_le_bytes([raw[4], raw[5]]) != AHEAD_VERSION {
        return Err(StoreError::Corrupt("txstat ahead version"));
    }
    let count = u32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]) as usize;
    if count == 0 {
        return Err(StoreError::Corrupt("txstat ahead count"));
    }
    let logical = u64::from_le_bytes([
        raw[12], raw[13], raw[14], raw[15], raw[16], raw[17], raw[18], raw[19],
    ]);
    let mut at = AHEAD_HEAD;
    let mut images = Vec::with_capacity(count);
    for _ in 0..count {
        if raw.len() < at + AHEAD_REC {
            return Err(StoreError::Corrupt("txstat ahead short"));
        }
        let mut off_buf = [0u8; 8];
        off_buf.copy_from_slice(&raw[at..at + 8]);
        let off = u64::from_le_bytes(off_buf);
        let sum = &raw[at + 8..at + 40];
        let image = &raw[at + 40..at + AHEAD_REC];
        if !off.is_multiple_of(4096) {
            return Err(StoreError::Corrupt("txstat ahead offset"));
        }
        let mut eng = sha256::HashEngine::default();
        eng.input(&off.to_le_bytes());
        eng.input(image);
        if sha256::Hash::from_engine(eng).to_byte_array() != *sum {
            return Err(StoreError::Corrupt("txstat ahead checksum"));
        }
        images.push((off, image));
        at += AHEAD_REC;
    }
    if at != raw.len() {
        return Err(StoreError::Corrupt("txstat ahead length"));
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| StoreError::io(path, e))?;
    for (off, image) in images {
        file.seek(SeekFrom::Start(off))
            .map_err(|e| StoreError::io(path, e))?;
        file.write_all(image).map_err(|e| StoreError::io(path, e))?;
    }
    let header = file::encode_leading_header(TableKind::TxStat, logical);
    file.seek(SeekFrom::Start(0))
        .map_err(|e| StoreError::io(path, e))?;
    file.write_all(&header)
        .map_err(|e| StoreError::io(path, e))?;
    file.durable_sync_all()
        .map_err(|e| StoreError::io(path, e))?;
    drop(file);
    std::fs::remove_file(&side).map_err(|e| StoreError::io(&side, e))?;
    if let Some(parent) = side.parent() {
        file::fsync_parent_dir(parent).map_err(|e| StoreError::io(parent, e))?;
    }
    Ok(())
}

#[doc(hidden)]
pub fn checked_blob_len(len: usize, remain: u64) -> Result<usize, StoreError> {
    if len > TXSTAT_BLOB_CEILING || (len as u64) > remain {
        return Err(StoreError::Corrupt("invariant: txstat blob length"));
    }
    Ok(len)
}

/// Packed confirm-time econ. All-zero on disk is unstamped (not this struct).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TxStatRow {
    pub fee_sat: u64,
    pub base: u32,
    pub wit_extra: u32,
}

impl TxStatRow {
    pub fn size(&self) -> u64 {
        u64::from(self.base).saturating_add(u64::from(self.wit_extra))
    }

    pub fn weight(&self) -> u64 {
        u64::from(self.base)
            .saturating_mul(4)
            .saturating_add(u64::from(self.wit_extra))
    }

    pub fn has_witness(&self) -> bool {
        self.wit_extra != 0
    }
}

pub fn encode_stream(row: &TxStatRow) -> Result<Vec<u8>, StoreError> {
    let mut v = Vec::new();
    write_uleb128(&mut v, row.fee_sat);
    write_uleb128(&mut v, u64::from(row.base));
    write_uleb128(&mut v, u64::from(row.wit_extra));
    Ok(v)
}

pub fn pack_cell(row: &TxStatRow) -> Result<([u8; 8], Option<Vec<u8>>), StoreError> {
    let s = encode_stream(row)?;
    let mut cell = [0u8; 8];
    if s.len() <= 8 {
        cell[..s.len()].copy_from_slice(&s);
        Ok((cell, None))
    } else {
        cell.copy_from_slice(&s[..8]);
        Ok((cell, Some(s[8..].to_vec())))
    }
}

#[doc(hidden)]
pub fn read_canonical_uleb(buf: &[u8]) -> Result<(u64, usize), StoreError> {
    let (v, n) = read_uleb128(buf).map_err(|e| StoreError::Corrupt(e.0))?;
    if n != uleb128_len(v) {
        return Err(StoreError::Corrupt("txstat uleb overlong"));
    }
    Ok((v, n))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellParse {
    Unstamped,
    Complete(TxStatRow),
    NeedTail,
}

pub fn parse_cell(cell: [u8; 8]) -> Result<CellParse, StoreError> {
    if cell == [0u8; 8] {
        return Ok(CellParse::Unstamped);
    }
    match parse_stream(&cell, true)? {
        StreamParse::Complete(row) => Ok(CellParse::Complete(row)),
        StreamParse::NeedMore => Ok(CellParse::NeedTail),
    }
}

#[doc(hidden)]
pub enum StreamParse {
    Complete(TxStatRow),
    NeedMore,
}

#[doc(hidden)]
pub fn parse_stream(buf: &[u8], allow_trunc: bool) -> Result<StreamParse, StoreError> {
    let mut off = 0usize;
    let mut fields = [0u64; 3];
    for slot in &mut fields {
        if off >= buf.len() {
            if allow_trunc {
                return Ok(StreamParse::NeedMore);
            }
            return Err(StoreError::Corrupt("txstat uleb truncated"));
        }
        match read_canonical_uleb(&buf[off..]) {
            Ok((v, n)) => {
                *slot = v;
                off += n;
            }
            Err(StoreError::Corrupt("uleb128 truncated")) if allow_trunc => {
                return Ok(StreamParse::NeedMore);
            }
            Err(e) => return Err(e),
        }
    }
    if buf[off..].iter().any(|&b| b != 0) {
        return Err(StoreError::Corrupt("txstat cell trailing non-zero"));
    }
    let base = u32::try_from(fields[1]).map_err(|_| StoreError::Corrupt("txstat base"))?;
    let wit_extra =
        u32::try_from(fields[2]).map_err(|_| StoreError::Corrupt("txstat wit_extra"))?;
    Ok(StreamParse::Complete(TxStatRow {
        fee_sat: fields[0],
        base,
        wit_extra,
    }))
}

pub fn parse_with_tail(cell: [u8; 8], tail: &[u8]) -> Result<TxStatRow, StoreError> {
    let mut buf = Vec::with_capacity(8 + tail.len());
    buf.extend_from_slice(&cell);
    buf.extend_from_slice(tail);
    match parse_stream(&buf, false)? {
        StreamParse::Complete(row) => Ok(row),
        StreamParse::NeedMore => Err(StoreError::Corrupt("invariant: txstat overflow missing")),
    }
}

pub fn encode_ovf_blob(tails: &[(u16, Vec<u8>)]) -> Result<Vec<u8>, StoreError> {
    let mut out = Vec::new();
    for (idx, rest) in tails {
        if rest.is_empty() {
            return Err(StoreError::Corrupt("txstat overflow empty tail"));
        }
        if rest.len() > 255 {
            return Err(StoreError::Corrupt("txstat overflow tail"));
        }
        out.extend_from_slice(&idx.to_le_bytes());
        out.push(rest.len() as u8);
        out.extend_from_slice(rest);
    }
    Ok(out)
}

pub fn decode_ovf_blob(raw: &[u8]) -> Result<Vec<(u16, Vec<u8>)>, StoreError> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off < raw.len() {
        if raw.len() - off < 3 {
            return Err(StoreError::Corrupt("txstat overflow short"));
        }
        let idx = u16::from_le_bytes([raw[off], raw[off + 1]]);
        let n = raw[off + 2] as usize;
        off += 3;
        if n == 0 || off + n > raw.len() {
            return Err(StoreError::Corrupt("txstat overflow tail"));
        }
        out.push((idx, raw[off..off + n].to_vec()));
        off += n;
    }
    Ok(out)
}

/// A stamped cell completed by its overflow tail, or `None` when the tail is
/// missing or does not complete it.
///
/// A stamp writes the cells, then the blob, then its `txstat.blk` slot, and
/// none is synced until the next barrier. After a power cut a cell can name
/// a tail that never landed. Stats are rebuilt from the block, so such a row
/// is unstamped: readers fall back and `getblockstats` stamps it again.
fn merge_tail(cell: [u8; 8], tails: &[(u16, Vec<u8>)], idx: u16) -> Option<TxStatRow> {
    let rest = tail_for_index(tails, idx).ok()?;
    parse_with_tail(cell, rest).ok()
}

#[doc(hidden)]
pub fn tail_for_index(tails: &[(u16, Vec<u8>)], idx: u16) -> Result<&[u8], StoreError> {
    match tails.binary_search_by_key(&idx, |(i, _)| *i) {
        Ok(p) => Ok(tails[p].1.as_slice()),
        Err(_) => Err(StoreError::Corrupt("invariant: txstat overflow missing")),
    }
}

/// Dense 8 B/create body plus per-header overflow tails.
pub struct TxStat {
    pub body: TableFile,
    pub ovf: TableFile,
    pub blk: TableFile,
    pub count: AtomicU64,
    pub ovf_len: AtomicU64,
    /// Serialises [`Self::put_header_blob`]'s read of the `ovf` end and its
    /// append. Readers stamp blocks too, so appends run on several threads.
    ovf_append: Mutex<()>,
    /// One journal+body write+sync+remove at a time. The IBD append and the
    /// read-side stamp can otherwise replace each other's `*.ahead` image.
    journal: Mutex<()>,
}

impl TxStat {
    /// Write the intended full pages before an in-place cell write.
    fn write_ahead_pages(&self, start: u64, bytes: &[u8]) -> Result<Option<PathBuf>, StoreError> {
        if bytes.is_empty() {
            return Ok(None);
        }
        let first = start & !4095;
        let last = (start + bytes.len() as u64 - 1) & !4095;
        let mut blocks: BTreeMap<u64, [u8; 4096]> = BTreeMap::new();
        let mut block = first;
        while block <= last {
            blocks.insert(block, read_page(self.body.path(), &self.body, block)?);
            block += 4096;
        }
        // The length write at offset 8 touches page 0 too.
        if let std::collections::btree_map::Entry::Vacant(entry) = blocks.entry(0) {
            entry.insert(read_page(self.body.path(), &self.body, 0)?);
        }
        for (at, byte) in bytes.iter().enumerate() {
            let off = start + at as u64;
            let Some(image) = blocks.get_mut(&(off & !4095)) else {
                continue;
            };
            image[(off & 4095) as usize] = *byte;
        }
        let mut buf = Vec::new();
        buf.extend_from_slice(AHEAD_MAGIC);
        buf.extend_from_slice(&AHEAD_VERSION.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&(blocks.len() as u32).to_le_bytes());
        buf.extend_from_slice(
            &self
                .body
                .logical_len()
                .max(start + bytes.len() as u64)
                .to_le_bytes(),
        );
        for (off, image) in &blocks {
            buf.extend_from_slice(&off.to_le_bytes());
            let mut eng = sha256::HashEngine::default();
            eng.input(&off.to_le_bytes());
            eng.input(image);
            buf.extend_from_slice(&sha256::Hash::from_engine(eng).to_byte_array());
            buf.extend_from_slice(image);
        }
        let side = ahead_path(self.body.path());
        file::write_synced_tmp_rename(&side, &buf)?;
        #[cfg(debug_assertions)]
        if take_fail_ahead(self.body.path()) {
            return Err(StoreError::io(
                self.body.path(),
                std::io::Error::other("injected txstat ahead failure"),
            ));
        }
        Ok(Some(side))
    }

    fn remove_ahead(side: Option<PathBuf>) -> Result<(), StoreError> {
        let Some(side) = side else {
            return Ok(());
        };
        std::fs::remove_file(&side).map_err(|e| StoreError::io(&side, e))?;
        if let Some(parent) = side.parent() {
            file::fsync_parent_dir(parent).map_err(|e| StoreError::io(parent, e))?;
        }
        Ok(())
    }

    pub fn flush(&self) -> Result<(), StoreError> {
        self.body.flush()?;
        self.ovf.flush()?;
        self.blk.flush()?;
        Ok(())
    }

    #[cfg(debug_assertions)]
    pub fn pending_sync(&self) -> bool {
        self.body.pending_sync() || self.ovf.pending_sync() || self.blk.pending_sync()
    }

    pub fn create(dir: &Path) -> Result<Self, StoreError> {
        let body = TableFile::create(Self::body_path(dir), TableKind::TxStat)?;
        let pad = vec![0u8; (TXSTAT_BODY_HEADER as usize).saturating_sub(FILE_HEADER_LEN)];
        if !pad.is_empty() {
            body.write_at_pwrite(FILE_HEADER_LEN as u64, &pad)?;
        }
        let ovf = TableFile::create(Self::ovf_path(dir), TableKind::TxStatOvf)?;
        ovf.set_grow_policy(GrowPolicy::Tight1MiB);
        let blk = TableFile::create(Self::blk_path(dir), TableKind::TxStatBlk)?;
        blk.set_grow_policy(GrowPolicy::Tight1MiB);
        let bpad = vec![0u8; (BLK_HEADER as usize).saturating_sub(FILE_HEADER_LEN)];
        if !bpad.is_empty() {
            blk.write_at_pwrite(FILE_HEADER_LEN as u64, &bpad)?;
        }
        Ok(Self {
            body,
            ovf,
            blk,
            count: AtomicU64::new(0),
            ovf_len: AtomicU64::new(0),
            ovf_append: Mutex::new(()),
            journal: Mutex::new(()),
        })
    }

    pub fn open(dir: &Path) -> Result<Self, StoreError> {
        let leftover = dir.join("txfixed.body");
        if leftover.exists() {
            std::fs::remove_file(&leftover).map_err(|e| StoreError::io(&leftover, e))?;
        }
        let body_path = Self::body_path(dir);
        restore_ahead(&body_path)?;
        let body = if body_path.exists() {
            TableFile::open(body_path, TableKind::TxStat)?
        } else {
            let b = TableFile::create(&body_path, TableKind::TxStat)?;
            let pad = vec![0u8; (TXSTAT_BODY_HEADER as usize).saturating_sub(FILE_HEADER_LEN)];
            if !pad.is_empty() {
                b.write_at_pwrite(FILE_HEADER_LEN as u64, &pad)?;
            }
            b
        };
        let len = body.logical_len();
        let count = if len <= TXSTAT_BODY_HEADER {
            0
        } else {
            (len - TXSTAT_BODY_HEADER) / TXSTAT_ENTRY_LEN
        };
        let ovf_path = Self::ovf_path(dir);
        let ovf = if ovf_path.exists() {
            let f = TableFile::open(ovf_path, TableKind::TxStatOvf)?;
            f.set_grow_policy(GrowPolicy::Tight1MiB);
            f
        } else {
            let f = TableFile::create(&ovf_path, TableKind::TxStatOvf)?;
            f.set_grow_policy(GrowPolicy::Tight1MiB);
            f
        };
        let blk_path = Self::blk_path(dir);
        let blk = if blk_path.exists() {
            let f = TableFile::open(blk_path, TableKind::TxStatBlk)?;
            f.set_grow_policy(GrowPolicy::Tight1MiB);
            f
        } else {
            let f = TableFile::create(&blk_path, TableKind::TxStatBlk)?;
            f.set_grow_policy(GrowPolicy::Tight1MiB);
            let bpad = vec![0u8; (BLK_HEADER as usize).saturating_sub(FILE_HEADER_LEN)];
            if !bpad.is_empty() {
                f.write_at_pwrite(FILE_HEADER_LEN as u64, &bpad)?;
            }
            f
        };
        let ovf_len = ovf.data_len();
        Ok(Self {
            body,
            ovf,
            blk,
            count: AtomicU64::new(count),
            ovf_len: AtomicU64::new(ovf_len),
            ovf_append: Mutex::new(()),
            journal: Mutex::new(()),
        })
    }

    #[doc(hidden)]
    pub fn body_path(dir: &Path) -> PathBuf {
        dir.join("txstat.body")
    }
    #[doc(hidden)]
    pub fn ovf_path(dir: &Path) -> PathBuf {
        dir.join("txstat.ovf")
    }
    #[doc(hidden)]
    pub fn blk_path(dir: &Path) -> PathBuf {
        dir.join("txstat.blk")
    }

    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Acquire)
    }

    pub fn truncate_to_count(&self, new_count: u64) -> Result<(), StoreError> {
        let cur = self.count();
        if new_count > cur {
            return Err(StoreError::Corrupt("txstat.body truncate past count"));
        }
        if new_count == cur {
            return Ok(());
        }
        let new_len = TXSTAT_BODY_HEADER + new_count * TXSTAT_ENTRY_LEN;
        self.body.set_logical_len(new_len)?;
        self.count.store(new_count, Ordering::Release);
        Ok(())
    }

    pub fn extend_or_truncate_to(&self, new_count: u64) -> Result<(), StoreError> {
        let cur = self.count();
        if new_count == cur {
            return Ok(());
        }
        if new_count < cur {
            return self.truncate_to_count(new_count);
        }
        let new_len = TXSTAT_BODY_HEADER + new_count * TXSTAT_ENTRY_LEN;
        self.body.set_logical_len(new_len)?;
        self.count.store(new_count, Ordering::Release);
        Ok(())
    }

    #[inline]
    pub fn entry_offset(fk: u64) -> Result<u64, StoreError> {
        if fk == 0 {
            return Err(StoreError::InvalidFk);
        }
        Ok(TXSTAT_BODY_HEADER + (fk - 1) * TXSTAT_ENTRY_LEN)
    }

    pub fn get_cell(&self, fk: Fk) -> Result<[u8; 8], StoreError> {
        let id = fk.get().ok_or(StoreError::InvalidFk)?;
        if id > self.count() {
            return Err(StoreError::NotFound);
        }
        let off = Self::entry_offset(id)?;
        let mut buf = [0u8; 8];
        self.body.read_at(off, &mut buf)?;
        Ok(buf)
    }

    pub fn get_row_merged(
        &self,
        fk: Fk,
        first_fk: u64,
        blob: &[u8],
    ) -> Result<Option<TxStatRow>, StoreError> {
        let cell = self.get_cell(fk)?;
        match parse_cell(cell)? {
            CellParse::Unstamped => Ok(None),
            CellParse::Complete(row) => Ok(Some(row)),
            CellParse::NeedTail => {
                let id = fk.get().ok_or(StoreError::InvalidFk)?;
                let Some(idx) = id.checked_sub(first_fk).and_then(|i| u16::try_from(i).ok()) else {
                    return Ok(None);
                };
                Ok(merge_tail(
                    cell,
                    &decode_ovf_blob(blob).unwrap_or_default(),
                    idx,
                ))
            }
        }
    }

    pub fn get_range(
        &self,
        first: u64,
        last: u64,
        blob: Option<&[u8]>,
    ) -> Result<Vec<Option<TxStatRow>>, StoreError> {
        if last < first {
            return Ok(Vec::new());
        }
        let n = self.count();
        if first == 0 || last > n {
            return Err(StoreError::NotFound);
        }
        let count = (last - first + 1) as usize;
        let off = Self::entry_offset(first)?;
        let mut raw = vec![0u8; count * TXSTAT_ENTRY_LEN as usize];
        let rc = crate::bulk_io::pread_single(self.body.read_fd(), off, &mut raw);
        if rc < 0 {
            return Err(StoreError::io(
                self.body.path(),
                std::io::Error::from_raw_os_error(-rc),
            ));
        }
        if (rc as usize) != raw.len() {
            self.body.pread_at(off, &mut raw)?;
        }
        let tails = match blob {
            Some(b) if !b.is_empty() => decode_ovf_blob(b).unwrap_or_default(),
            _ => Vec::new(),
        };
        let (cells, _) = raw.as_chunks::<{ TXSTAT_ENTRY_LEN as usize }>();
        let mut out = Vec::with_capacity(count);
        for (i, cell) in cells.iter().enumerate() {
            match parse_cell(*cell)? {
                CellParse::Unstamped => out.push(None),
                CellParse::Complete(row) => out.push(Some(row)),
                CellParse::NeedTail => {
                    let idx = u16::try_from(i)
                        .map_err(|_| StoreError::Corrupt("txstat overflow index"))?;
                    out.push(merge_tail(*cell, &tails, idx));
                }
            }
        }
        Ok(out)
    }

    pub fn append_batch(
        &self,
        base_count: u64,
        rows: &[TxStatRow],
    ) -> Result<Vec<(u64, Vec<u8>)>, StoreError> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let _journal = self.journal.lock().unwrap_or_else(|e| e.into_inner());
        let cur = self.count.load(Ordering::Acquire);
        if cur != base_count {
            return Err(StoreError::Corrupt("txstat.body count mismatch on append"));
        }
        let start = TXSTAT_BODY_HEADER + base_count * TXSTAT_ENTRY_LEN;
        let mut blob = Vec::with_capacity(rows.len() * TXSTAT_ENTRY_LEN as usize);
        let mut tails = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            let (cell, tail) = pack_cell(row)?;
            blob.extend_from_slice(&cell);
            if let Some(t) = tail {
                tails.push((base_count + 1 + i as u64, t));
            }
        }
        let side = self.write_ahead_pages(start, &blob)?;
        self.body.write_at_pwrite(start, &blob)?;
        self.body.sync_data_only()?;
        Self::remove_ahead(side)?;
        let new = base_count + rows.len() as u64;
        self.count.store(new, Ordering::Release);
        Ok(tails)
    }

    pub fn write_row(&self, fk: Fk, row: &TxStatRow) -> Result<(), StoreError> {
        let _journal = self.journal.lock().unwrap_or_else(|e| e.into_inner());
        let id = fk.get().ok_or(StoreError::InvalidFk)?;
        if id > self.count() {
            return Err(StoreError::NotFound);
        }
        let (cell, tail) = pack_cell(row)?;
        if tail.is_some() {
            return Err(StoreError::Corrupt("txstat overflow needs header blob"));
        }
        let off = Self::entry_offset(id)?;
        let side = self.write_ahead_pages(off, &cell)?;
        self.body.write_at_pwrite(off, &cell)?;
        self.body.sync_data_only()?;
        Self::remove_ahead(side)?;
        Ok(())
    }

    pub fn write_block_rows(
        &self,
        header_fk: Fk,
        first_fk: u64,
        rows: &[TxStatRow],
    ) -> Result<(), StoreError> {
        let _journal = self.journal.lock().unwrap_or_else(|e| e.into_inner());
        if rows.is_empty() {
            return self.put_header_blob(header_fk, &[]);
        }
        let last = first_fk
            .checked_add(rows.len() as u64 - 1)
            .ok_or(StoreError::Corrupt("txstat block last fk"))?;
        if last > self.count() {
            return Err(StoreError::NotFound);
        }
        let start = Self::entry_offset(first_fk)?;
        let mut blob = Vec::with_capacity(rows.len() * 8);
        let mut tails = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            let (cell, tail) = pack_cell(row)?;
            blob.extend_from_slice(&cell);
            if let Some(t) = tail {
                let idx =
                    u16::try_from(i).map_err(|_| StoreError::Corrupt("txstat overflow index"))?;
                tails.push((idx, t));
            }
        }
        let side = self.write_ahead_pages(start, &blob)?;
        self.body.write_at_pwrite(start, &blob)?;
        self.body.sync_data_only()?;
        Self::remove_ahead(side)?;
        let raw = encode_ovf_blob(&tails)?;
        self.put_header_blob(header_fk, &raw)
    }

    pub fn put_overflows_for_headers(
        &self,
        ranges: &[(Fk, Fk, u32)],
        tails: &[(u64, Vec<u8>)],
    ) -> Result<(), StoreError> {
        if ranges.is_empty() {
            if !tails.is_empty() {
                return Err(StoreError::Corrupt("txstat overflow without header"));
            }
            return Ok(());
        }
        for &(hfk, first, n) in ranges {
            let Some(first_id) = first.get() else {
                continue;
            };
            let last = first_id.saturating_add(u64::from(n.saturating_sub(1)));
            let mut chunk = Vec::new();
            for (fk, rest) in tails {
                if *fk < first_id || *fk > last {
                    continue;
                }
                let idx = u16::try_from(*fk - first_id)
                    .map_err(|_| StoreError::Corrupt("txstat overflow index"))?;
                chunk.push((idx, rest.clone()));
            }
            if chunk.is_empty() {
                continue;
            }
            let raw = encode_ovf_blob(&chunk)?;
            self.put_header_blob(hfk, &raw)?;
        }
        Ok(())
    }

    pub fn put_header_blob(&self, header_fk: Fk, blob: &[u8]) -> Result<(), StoreError> {
        let hfk = header_fk.get().ok_or(StoreError::InvalidFk)?;
        let (off, len, n) = if blob.is_empty() {
            (0u64, 0u32, 0u32)
        } else {
            let _append = self.ovf_append.lock().unwrap_or_else(|e| e.into_inner());
            let at = FILE_HEADER_LEN as u64 + self.ovf.data_len();
            self.ovf.write_at_pwrite(at, blob)?;
            let nlen = blob.len() as u64;
            let n_ovf = decode_ovf_blob(blob)?.len() as u32;
            (
                at,
                u32::try_from(nlen).map_err(|_| StoreError::Corrupt("txstat ovf len"))?,
                n_ovf,
            )
        };
        let slot_off = BLK_HEADER + (hfk - 1) * BLK_SLOT;
        let mut slot = [0u8; 16];
        slot[0..8].copy_from_slice(&off.to_le_bytes());
        slot[8..12].copy_from_slice(&len.to_le_bytes());
        slot[12..16].copy_from_slice(&n.to_le_bytes());
        self.blk.write_at_pwrite(slot_off, &slot)?;
        Ok(())
    }

    /// [`Self::header_blob`] for a reader: a slot past the end of
    /// `txstat.ovf` is a stamp whose blob never landed, and reads as no
    /// blob. An IO error still fails.
    pub fn header_blob_or_empty(&self, header_fk: Fk) -> Result<Vec<u8>, StoreError> {
        match self.header_blob(header_fk) {
            Err(StoreError::Corrupt(_)) => Ok(Vec::new()),
            other => other,
        }
    }

    pub fn header_blob(&self, header_fk: Fk) -> Result<Vec<u8>, StoreError> {
        let hfk = header_fk.get().ok_or(StoreError::InvalidFk)?;
        let slot_off = BLK_HEADER + (hfk - 1) * BLK_SLOT;
        if slot_off + BLK_SLOT > self.blk.logical_len() {
            return Ok(Vec::new());
        }
        let mut slot = [0u8; 16];
        self.blk.read_at(slot_off, &mut slot)?;
        let off = le_u64!(slot, 0);
        let len = le_u32!(slot, 8) as usize;
        if len == 0 {
            return Ok(Vec::new());
        }
        let remain = self.ovf.logical_len().saturating_sub(off);
        let len = checked_blob_len(len, remain)?;
        let mut buf = vec![0u8; len];
        self.ovf.read_at(off, &mut buf)?;
        Ok(buf)
    }
}
