use bitcoin::hashes::Hash;
use bitcoin::Wtxid;
use rbitcoin_net::inv_bucket::*;

#[test]
fn count_floor_is_the_old_broadcast_target() {
    assert_eq!(COUNT_FLOOR, -70.0);
}

#[test]
fn first_refill_only_sets_the_baseline() {
    let mut bucket = TokenBucket::new(14.0, 1.0, 420.0, -70.0);
    bucket.refill(1_000);
    assert_eq!(bucket.value(), 1.0);
    bucket.refill(2_000);
    assert_eq!(bucket.value(), 15.0);
    bucket.refill(100_000);
    assert_eq!(bucket.value(), 420.0);
}

#[test]
fn enqueue_dedupes_a_wtxid_already_in_the_backlog() {
    let mut buckets = InvBuckets::new();
    let dir = InvDirection::Inbound;
    let w = Wtxid::from_byte_array([7; 32]);
    buckets.bucket_mut(dir).enqueue(w, 100);
    buckets.bucket_mut(dir).enqueue(w, 100);
    assert_eq!(
        buckets.backlog_len(dir),
        1,
        "one tx spends one pair of tokens"
    );
    assert_eq!(
        buckets.bucket_mut(dir).dequeue().map(|(got, _)| got),
        Some(w)
    );
    buckets.bucket_mut(dir).enqueue(w, 100);
    assert_eq!(
        buckets.backlog_len(dir),
        1,
        "a later rebroadcast may re-enqueue"
    );
}

#[test]
fn decrement_can_go_below_zero_down_to_the_floor() {
    let mut bucket = TokenBucket::new(14.0, 0.5, 420.0, -70.0);
    assert!(bucket.decrement(70.0));
    assert_eq!(bucket.value(), -69.5);
    assert!(!bucket.decrement(1.0));
    assert_eq!(bucket.value(), -70.5);
}
