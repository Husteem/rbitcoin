use rbitcoin_primitives::Fk;
use rbitcoin_query::confirm_run::head_drain::*;
use rbitcoin_store::StoreError;
use std::sync::{Arc, Mutex};

fn one_batch() -> Vec<HeadInsert> {
    vec![([0x11; 32], Fk(1))]
}

#[test]
fn join_restore_returns_the_batch_unless_insert_returns_ok() {
    // A panicking insert leaves the parked batch for the write thread.
    let restore = Arc::new(Mutex::new(Some(one_batch())));
    let mut handle = submit_head_drain(|| panic!("insert"));
    handle.restore = Some(Arc::clone(&restore));
    let (result, queued) = handle.join_restore();
    assert!(result.is_err(), "{result:?}");
    assert_eq!(queued, one_batch());

    // An Err insert also leaves it parked.
    let restore = Arc::new(Mutex::new(Some(one_batch())));
    let mut handle = submit_head_drain(|| Err(StoreError::Corrupt("insert failed")));
    handle.restore = Some(Arc::clone(&restore));
    let (result, queued) = handle.join_restore();
    assert!(result.is_err(), "{result:?}");
    assert_eq!(queued, one_batch());

    // Ok clears the parked batch.
    let restore = Arc::new(Mutex::new(Some(one_batch())));
    let restore_job = Arc::clone(&restore);
    let mut handle = submit_head_drain(move || {
        restore_job.lock().unwrap_or_else(|p| p.into_inner()).take();
        Ok(1)
    });
    handle.restore = Some(restore);
    let (result, queued) = handle.join_restore();
    assert_eq!(result.unwrap(), 1);
    assert!(queued.is_empty());
}
