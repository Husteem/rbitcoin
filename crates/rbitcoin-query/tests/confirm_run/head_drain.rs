use rbitcoin_primitives::Fk;
use rbitcoin_query::confirm_run::head_drain::*;
use rbitcoin_store::StoreError;
use std::sync::{Arc, Mutex};

fn one_batch() -> Vec<HeadInsert> {
    vec![([0x11; 32], Fk(1))]
}

#[test]
fn join_restore_returns_the_batch_unless_insert_returns_ok() {
    let restore = Arc::new(Mutex::new(None));
    let restore_job = Arc::clone(&restore);
    let mut handle =
        submit_head_drain(move || insert_parked(&restore_job, one_batch(), |_| panic!("insert")));
    handle.restore = Some(restore);
    let (result, queued) = handle.join_restore();
    assert!(result.is_err(), "{result:?}");
    assert_eq!(queued, one_batch());

    let restore = Arc::new(Mutex::new(None));
    let restore_job = Arc::clone(&restore);
    let mut handle = submit_head_drain(move || {
        insert_parked(&restore_job, one_batch(), |_| {
            Err(StoreError::Corrupt("insert failed"))
        })
    });
    handle.restore = Some(restore);
    let (result, queued) = handle.join_restore();
    assert!(result.is_err(), "{result:?}");
    assert_eq!(queued, one_batch());

    let restore = Arc::new(Mutex::new(None));
    let restore_job = Arc::clone(&restore);
    let mut handle = submit_head_drain(move || insert_parked(&restore_job, one_batch(), |_| Ok(1)));
    handle.restore = Some(restore);
    let (result, queued) = handle.join_restore();
    assert_eq!(result.unwrap(), 1);
    assert!(queued.is_empty());
}
