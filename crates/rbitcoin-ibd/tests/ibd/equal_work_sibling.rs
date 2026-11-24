//! A `--connect` peer announces block A on top of a short chain, serves A only
//! as a mutated copy, then announces A's sibling B with the same parent and
//! the same work. Core moves a peer's best known block on equal work
//! (`UpdateBlockAvailability`), so `FindNextBlocksToDownload` asks for B.
use bitcoin::hashes::Hash as _;
use bitcoin::p2p::{message::NetworkMessage, message_blockdata::Inventory, Magic};
use bitcoin::{
    absolute::LockTime, transaction::Version, Amount, Block, BlockHash, OutPoint, ScriptBuf,
    Sequence, Transaction, TxIn, TxOut, Txid, Witness,
};
use rbitcoin_consensus::{ChainParams, Milestone};
use rbitcoin_ibd::ibd::IbdConfig;
use rbitcoin_net::peer::{connect_and_handshake_timed, HandshakePolicy};
use rbitcoin_net::service::P2PNode;
use rbitcoin_net::v2::{read_v2_frame, write_v2_msg};
use rbitcoin_query::Query;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;

const CHAIN: usize = 3;
/// Getdata for A after B was announced, before the peer stops waiting for B.
const A_ASKS_AFTER_B: usize = 8;
/// Getdata for B before the peer stops waiting for B. Same budget as A: a
/// healthy sibling is asked for once and answered.
const B_ASKS_BEFORE_GIVE_UP: usize = 8;
/// A's ask count after which the peer re-announces the abandoned A. The third
/// copy is the one that abandons it.
const REANNOUNCE_AFTER_A_ASKS: usize = 3;

/// The sibling body the peer serves for B.
#[derive(Clone, Copy, PartialEq)]
enum Sibling {
    /// A valid block: catch-up must move to B.
    Valid,
    /// A block with a valid merkle root and header but a tx spending a null
    /// outpoint (`bad-txns-prevout-null`): catch-up must still end, at the
    /// chain tip, instead of re-asking for mutated A forever.
    Invalid,
    /// B's own body arrives mutated every time, like A's: the ask lane for B
    /// must be dropped too, or the same loop just moves one header over.
    Mutated,
    /// B is invalid like [`Sibling::Invalid`], and the peer re-announces the
    /// abandoned A in later `getheaders` replies. A stored reply that continues
    /// the cut-back walk tip must not put A back on the path and queue it.
    InvalidReannounced,
}

struct Fixture {
    chain: Vec<Block>,
    a: Block,
    b: Block,
    bad_b: Block,
}

impl Fixture {
    fn new() -> Self {
        let chain = crate::frontier_replay::chain(CHAIN as u32);
        let parent = chain[CHAIN].header;
        let height = CHAIN as u32 + 1;
        let a =
            rbitcoin_consensus::mine_empty_regtest(parent.block_hash(), parent.time + 600, height);
        let b =
            rbitcoin_consensus::mine_empty_regtest(parent.block_hash(), parent.time + 601, height);
        let bad_b = rbitcoin_consensus::mine_regtest_paying(
            parent.block_hash(),
            parent.time + 602,
            height,
            ScriptBuf::from_bytes(vec![rbitcoin_primitives::OP_TRUE]),
            vec![Transaction {
                version: Version::TWO,
                lock_time: LockTime::ZERO,
                input: vec![TxIn {
                    previous_output: OutPoint {
                        txid: Txid::all_zeros(),
                        vout: 0,
                    },
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::MAX,
                    witness: Witness::new(),
                }],
                output: vec![TxOut {
                    value: Amount::from_sat(1),
                    script_pubkey: ScriptBuf::from_bytes(vec![rbitcoin_primitives::OP_TRUE]),
                }],
            }],
        );
        assert_eq!(a.header.work(), b.header.work());
        assert_eq!(a.header.work(), bad_b.header.work());
        Self { chain, a, b, bad_b }
    }

    /// The header the peer announces for B.
    fn announced(&self, kind: Sibling) -> &Block {
        match kind {
            Sibling::Valid | Sibling::Mutated => &self.b,
            Sibling::Invalid | Sibling::InvalidReannounced => &self.bad_b,
        }
    }

    /// The body the peer serves when B is asked for.
    fn body(&self, kind: Sibling) -> Block {
        match kind {
            Sibling::Valid => self.b.clone(),
            Sibling::Invalid | Sibling::InvalidReannounced => self.bad_b.clone(),
            Sibling::Mutated => {
                let mut block = self.b.clone();
                let out = &mut block.txdata[0].output[0];
                out.value = out.value.checked_sub(Amount::from_sat(1)).unwrap();
                block
            }
        }
    }

    /// Same header as A, so the same hash, with a coinbase the merkle root
    /// does not commit to (`bad-txnmrklroot`).
    fn mutated_a(&self) -> Block {
        let mut block = self.a.clone();
        let out = &mut block.txdata[0].output[0];
        out.value = out.value.checked_sub(Amount::from_sat(1)).unwrap();
        block
    }

    fn chain_height(&self, hash: &BlockHash) -> Option<usize> {
        (1..=CHAIN).find(|&h| self.chain[h].block_hash() == *hash)
    }
}

#[derive(Debug, Default)]
struct Seen {
    a_asks: usize,
    a_asks_after_b: usize,
    a_reannounced: usize,
    b_asks: usize,
    b_asked: bool,
    gave_up: bool,
}

/// One manual peer on one connection. The first `getheaders` reply carries the
/// chain and A in one batch, so A is the walk tip whichever decode task runs
/// first. A is served mutated every time, and B is announced after the second
/// mutated copy. Gives up when the node keeps asking for A after B was
/// announced.
async fn serve(
    listener: TcpListener,
    fx: &Fixture,
    sibling: Sibling,
    seen: &Mutex<Seen>,
) -> Result<(), String> {
    let (stream, remote) = listener.accept().await.map_err(|e| e.to_string())?;
    let local = stream.local_addr().map_err(|e| e.to_string())?;
    let (_, mut reader, mut writer, _, _shutdown) = connect_and_handshake_timed(
        Duration::from_secs(5),
        stream,
        Magic::REGTEST,
        local,
        remote,
        CHAIN as i32 + 1,
        true,
        "/equal-work-sibling/",
        HandshakePolicy::plain(),
    )
    .await
    .map_err(|e| e.to_string())?;
    let a_hash = fx.a.block_hash();
    let b_hash = fx.announced(sibling).block_hash();
    let mut announced = false;
    let mut b_announced = false;
    let mut reannounced = 0usize;
    loop {
        let frame = read_v2_frame(&mut reader, Magic::REGTEST)
            .await
            .map_err(|e| e.to_string())?;
        let message = rbitcoin_net::msg_decode::decode_framed_offload(frame)
            .await
            .map_err(|e| e.to_string())?;
        let mut send = Vec::new();
        let mut stop = false;
        {
            let mut seen = seen.lock().unwrap();
            match message.payload() {
                NetworkMessage::GetHeaders(request) => {
                    let reannounce = sibling == Sibling::InvalidReannounced
                        && b_announced
                        && seen.a_asks >= REANNOUNCE_AFTER_A_ASKS
                        && reannounced < 2;
                    if reannounce {
                        send.push(NetworkMessage::Headers(vec![fx.a.header]));
                        reannounced += 1;
                        seen.a_reannounced = reannounced;
                    } else if announced {
                        send.push(NetworkMessage::Headers(Vec::new()));
                    } else {
                        let start = (0..=CHAIN)
                            .rev()
                            .find(|&h| request.locator_hashes.contains(&fx.chain[h].block_hash()))
                            .map_or(1, |h| h + 1);
                        let mut headers: Vec<_> =
                            fx.chain[start..].iter().map(|b| b.header).collect();
                        headers.push(fx.a.header);
                        send.push(NetworkMessage::Headers(headers));
                        announced = true;
                    }
                }
                NetworkMessage::GetData(inventory) => {
                    for item in inventory {
                        let (Inventory::Block(hash) | Inventory::WitnessBlock(hash)) = item else {
                            continue;
                        };
                        if let Some(h) = fx.chain_height(hash) {
                            send.push(NetworkMessage::Block(fx.chain[h].clone()));
                        } else if *hash == a_hash {
                            seen.a_asks += 1;
                            if b_announced {
                                seen.a_asks_after_b += 1;
                            }
                            send.push(NetworkMessage::Block(fx.mutated_a()));
                            if seen.a_asks == 2 {
                                send.push(NetworkMessage::Headers(vec![
                                    fx.announced(sibling).header,
                                ]));
                                b_announced = true;
                            }
                        } else if *hash == b_hash {
                            seen.b_asked = true;
                            seen.b_asks += 1;
                            send.push(NetworkMessage::Block(fx.body(sibling)));
                        }
                    }
                }
                NetworkMessage::Ping(nonce) => send.push(NetworkMessage::Pong(*nonce)),
                _ => {}
            }
            if seen.a_asks_after_b >= A_ASKS_AFTER_B || seen.b_asks >= B_ASKS_BEFORE_GIVE_UP {
                seen.gave_up = true;
                stop = true;
            }
        }
        if stop {
            return Ok(());
        }
        for msg in send {
            write_v2_msg(&mut writer, msg)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
}

struct Outcome {
    seen: Seen,
    synced: Result<u32, String>,
    tip: Option<BlockHash>,
    fx: Fixture,
}

async fn run(sibling: Sibling) -> Outcome {
    let dir = rbitcoin_store::testutil::TempDir::labeled("equal-work-sibling").unwrap();
    let node = P2PNode::start(
        "127.0.0.1:0".parse().unwrap(),
        Query::open_or_create_tiny(dir.join("store")).unwrap(),
        ChainParams::regtest(),
        Milestone::NONE,
        rbitcoin_net::REDIAL_INTERVAL_TEST,
    )
    .await
    .unwrap();
    let fx = Fixture::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = rbitcoin_net::NetAddr::Ip(listener.local_addr().unwrap());
    let mut cfg = IbdConfig::for_test();
    cfg.target_peers = 1;
    cfg.connect = vec![addr];
    cfg.stall = Duration::from_secs(60);
    let addrs = [addr];
    let seen = Mutex::new(Seen::default());
    let cancel = Arc::new(AtomicBool::new(false));
    let sync = rbitcoin_ibd::sync_cancellable(&node, &addrs, cfg, Some(cancel.clone()));
    let peer = async {
        let _ = serve(listener, &fx, sibling, &seen).await;
        cancel.store(true, Ordering::SeqCst);
    };
    let (synced, ()) =
        tokio::time::timeout(Duration::from_secs(60), async { tokio::join!(sync, peer) })
            .await
            .expect("catch-up ends, or the scripted peer gives up and cancels it");
    let tip = node.hub.tip_hash();
    node.shutdown().await;
    Outcome {
        seen: seen.into_inner().unwrap(),
        synced: synced.map_err(|e| e.to_string()),
        tip,
        fx,
    }
}

/// B is asked for although A keeps arriving mutated, from the same manual
/// connection that sent those copies (Core keeps a manual peer). B connects
/// and catch-up ends.
#[tokio::test(flavor = "current_thread")]
async fn equal_work_sibling_is_asked_while_the_tip_block_arrives_mutated() {
    let out = run(Sibling::Valid).await;
    assert!(
        out.seen.b_asked,
        "IBD must send a getdata for the equal-work sibling B: {:?}",
        out.seen
    );
    assert!(out.seen.a_asks >= 2, "{:?}", out.seen);
    assert!(!out.seen.gave_up, "{:?}", out.seen);
    assert!(out.synced.is_ok(), "{:?}", out.synced);
    assert_eq!(out.tip, Some(out.fx.b.block_hash()));
}

/// The reported incident: B's body is consensus-invalid, so no header can
/// advance the tip past 201. Catch-up must still end instead of re-asking for
/// mutated A forever, and once the copies are clearly not a fluke the node
/// must stop asking for A.
#[tokio::test(flavor = "current_thread")]
async fn an_invalid_equal_work_sibling_ends_catch_up_instead_of_re_asking_mutated_a() {
    let out = run(Sibling::Invalid).await;
    assert!(out.seen.b_asked, "{:?}", out.seen);
    assert!(!out.seen.gave_up, "{:?}", out.seen);
    assert!(
        out.seen.a_asks_after_b <= 4,
        "the node must stop asking for mutated A (the give-up threshold is \
         {A_ASKS_AFTER_B}; the exact count depends on how many copies were in \
         flight when the sibling was announced): {:?}",
        out.seen
    );
    assert!(out.synced.is_ok(), "{:?}", out.synced);
    assert_eq!(out.tip, Some(out.fx.chain[CHAIN].block_hash()));
}

/// A peer that also serves B mutated must not keep its ask lane: the sibling
/// is abandoned after the same bound, catch-up ends, and neither A nor B is
/// asked for forever.
#[tokio::test(flavor = "current_thread")]
async fn a_mutated_equal_work_sibling_stops_being_asked() {
    let out = run(Sibling::Mutated).await;
    assert!(out.seen.b_asked, "{:?}", out.seen);
    assert!(!out.seen.gave_up, "{:?}", out.seen);
    assert!(
        out.seen.b_asks <= 4,
        "the node must stop asking for mutated B: {:?}",
        out.seen
    );
    assert!(out.synced.is_ok(), "{:?}", out.synced);
    assert_eq!(out.tip, Some(out.fx.chain[CHAIN].block_hash()));
}

/// The abandoned header must stay off the path when the kept peer announces it
/// again: a stored reply that continues the cut-back walk tip is not a new
/// candidate, and asking for its body only produces another mutated copy.
#[tokio::test(flavor = "current_thread")]
async fn a_reannounced_abandoned_header_is_not_asked_again() {
    let out = run(Sibling::InvalidReannounced).await;
    assert!(out.seen.b_asked, "{:?}", out.seen);
    assert!(
        out.seen.a_reannounced >= 1,
        "the fixture must re-announce A after the abandon: {:?}",
        out.seen
    );
    assert!(
        out.seen.a_asks <= REANNOUNCE_AFTER_A_ASKS,
        "the re-announced abandoned header must not be asked for again: {:?}",
        out.seen
    );
    assert!(!out.seen.gave_up, "{:?}", out.seen);
    assert!(out.synced.is_ok(), "{:?}", out.synced);
    assert_eq!(out.tip, Some(out.fx.chain[CHAIN].block_hash()));
}
