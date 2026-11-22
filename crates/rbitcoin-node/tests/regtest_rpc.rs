use bitcoin::ScriptBuf;
use rbitcoin_consensus::{ChainParams, Milestone};
use rbitcoin_net::ChainHub;
use rbitcoin_query::Query;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// One block through the HTTP `generate*` mine.
fn generate_one(hub: &Arc<ChainHub>) -> Vec<bitcoin::BlockHash> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(hub.generate_to_script_for_rpc_async(
            1,
            ScriptBuf::from_bytes(vec![0x51]),
            vec![],
            true,
        ))
        .unwrap()
}

#[test]
fn regtest_generate_one() {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rbitcoin-node-gen-{n}"));
    std::fs::create_dir_all(&dir).unwrap();
    let hub = Arc::new(ChainHub::new(
        Query::open_or_create_tiny(dir.join("store")).unwrap(),
        ChainParams::regtest(),
        Milestone::NONE,
    ));
    let hashes = generate_one(&hub);
    assert_eq!(hashes.len(), 1);
    assert_eq!(hub.tip_height(), Some(1));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn op_true_signet_generate_one() {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rbitcoin-node-signet-gen-{n}"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut params = ChainParams::regtest();
    params.signet_challenge = Some(ScriptBuf::from_bytes(vec![0x51]));
    let hub = Arc::new(ChainHub::new(
        Query::open_or_create_tiny(dir.join("store")).unwrap(),
        params,
        Milestone::NONE,
    ));
    let hashes = generate_one(&hub);
    assert_eq!(hashes.len(), 1);
    assert_eq!(hub.tip_height(), Some(1));
    let bits = hub.tip_header().expect("tip").bits;
    assert_eq!(
        bits.to_consensus(),
        rbitcoin_consensus::REGTEST_POW_BITS,
        "the signet miner must grind the parent target, which is regtest here"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--test=bip94` turns BIP94 on for regtest, as Core's `-test=bip94` does:
/// a period's first block more than 600 s before its parent is
/// `time-timewarp-attack`. Without the switch the same block is valid.
#[test]
fn regtest_test_bip94_refuses_a_timewarped_period_start() {
    use rbitcoin_node::cli::{operator_config_from_args, OperatorArgs};
    use rbitcoin_node::config::NodeEnv;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let op_true = || ScriptBuf::from_bytes(vec![0x51]);
    for bip94 in [false, true] {
        let mut argv = vec!["rbitcoin-node", "--network=regtest"];
        if bip94 {
            argv.push("--test=bip94");
        }
        let mut params = match operator_config_from_args(argv, &NodeEnv::default()) {
            Ok(OperatorArgs::Ready { config, .. }) => config.chain_params().unwrap(),
            other => panic!("expected assembled config, got {other:?}"),
        };
        // A four-block period, so three mined blocks reach a period start
        // instead of 143. With three, median-time-past stays more than 601 s
        // below the tip.
        params.btc.pow_target_timespan = 4 * params.btc.pow_target_spacing;
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rbitcoin-node-bip94-{bip94}-{n}"));
        std::fs::create_dir_all(&dir).unwrap();
        let hub = Arc::new(ChainHub::new(
            Query::open_or_create_tiny(dir.join("store")).unwrap(),
            params,
            Milestone::NONE,
        ));
        hub.ensure_genesis().unwrap();
        let period = hub.params.difficulty_adjustment_interval();
        assert_eq!(period, 4);
        for height in 1..period {
            hub.clock.set_mock(1_700_000_000 + 2000 * i64::from(height));
            rt.block_on(hub.generate_to_script_for_rpc_async(1, op_true(), vec![], true))
                .unwrap();
        }
        assert_eq!(hub.tip_height(), Some(period - 1));
        let tip = hub.header_of(&hub.tip_hash().unwrap()).unwrap();
        let mut warped = hub.assemble_block_to_script(op_true(), vec![]).unwrap();
        warped.header.time = tip.time - 601;
        let out = hub.check_block_proposal(&warped);
        let _ = std::fs::remove_dir_all(&dir);
        if bip94 {
            assert_eq!(out.unwrap_err(), "time-timewarp-attack");
        } else {
            assert!(out.is_ok(), "BIP94 is off without --test: {out:?}");
        }
    }
}
