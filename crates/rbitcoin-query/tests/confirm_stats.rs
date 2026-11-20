use rbitcoin_query::confirm_stats::*;

#[test]
fn spend_checkpoint_window_counts_max_and_notes() {
    let stats = ConfirmStats::default();
    stats.note_spend_checkpoint(10, 100);
    stats.note_spend_checkpoint(20, 200);
    let w = stats.take_window();
    assert_eq!(w.spend_ckpt_n, 2);
    assert_eq!(w.spend_ckpt_max_ms, 20);
    assert_eq!(w.spend_notes_ram, 200);
    let w = stats.take_window();
    assert_eq!(w.spend_ckpt_n, 0);
    assert_eq!(w.spend_ckpt_max_ms, 0);
    assert_eq!(w.spend_notes_ram, 0);
}

#[test]
fn last_union_miss_roundtrips_miss_on_codes() {
    let stats = ConfirmStats::default();
    assert_eq!(stats.last_union_miss().n, 0);
    let tid = [7u8; 32];
    for (on, want) in [
        (Some("head"), Some("head")),
        (Some("body"), Some("body")),
        (Some("idx"), Some("idx")),
        (Some("fence"), Some("fence")),
        (Some("nope"), None),
        (None, None),
    ] {
        stats.note_union_miss(tid, 3, true, on, 9);
        let got = stats.last_union_miss();
        assert_eq!(got.n, 3);
        assert!(got.pending);
        assert_eq!(got.txid, Some(tid));
        assert_eq!(got.miss_on, want, "on={on:?}");
        assert_eq!(got.miss_cands, 9);
    }
}
