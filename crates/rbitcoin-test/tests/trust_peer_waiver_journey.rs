//! `--trust-peer` warn-and-apply, seen by the operator of one node.
//!
//! The node is the operator CLI (`journey-node`) with `--trust-peer` naming a
//! scripted listener. Its stderr is the operator log. The listener speaks P2P
//! on each socket the node dials (catch-up, then tip follow) and serves one
//! regtest chain with faults the waiver covers.

use bitcoin::block::Header;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::p2p::message::NetworkMessage;
use bitcoin::p2p::message_blockdata::Inventory;
use bitcoin::p2p::Magic;
use bitcoin::{Amount, Block, BlockHash, ScriptBuf};
use rbitcoin_consensus::{mine_empty_regtest, prepare_regtest_candidate};
use rbitcoin_net::peer::{connect_and_handshake, HandshakePolicy};
use rbitcoin_net::v2::{read_v2_frame, write_v2_msg};
use rbitcoin_test::mine::regtest_genesis;
use rbitcoin_test::TestDatadir;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

fn llvm_cov_wall(default_secs: u64, llvm_secs: u64) -> Duration {
    if std::env::var_os("CARGO_LLVM_COV").is_some() {
        Duration::from_secs(llvm_secs)
    } else {
        Duration::from_secs(default_secs)
    }
}

async fn jsonrpc(addr: SocketAddr, method: &str, params: Value) -> Value {
    rbitcoin_test::jsonrpc::jsonrpc(addr, method, params).await
}

fn write_token(dir: &Path) {
    std::fs::write(dir.join("rpc.token"), "pass\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.join("rpc.token"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
}

/// `rbitcoin-node` child. Stderr is the operator log.
struct NodeProc {
    child: Option<Child>,
    stderr: Arc<Mutex<String>>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl NodeProc {
    fn log(&self) -> String {
        self.stderr.lock().unwrap().clone()
    }
}

impl Drop for NodeProc {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn spawn_trusting_node(dir: &Path, peer: SocketAddr) -> NodeProc {
    // Cargo 1.95 keeps the hyphen in `CARGO_BIN_EXE_journey-node`.
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_journey-node"));
    let mut child = Command::new(bin)
        .arg("--datadir")
        .arg(dir)
        .arg("--network")
        .arg("regtest")
        .arg("--trust-peer")
        .arg(peer.to_string())
        .arg("--rpc-listen")
        .arg("127.0.0.1:0")
        .arg("--log-level")
        .arg("info")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn rbitcoin-node: {e}"));
    let pipe = child.stderr.take().expect("stderr piped");
    let stderr = Arc::new(Mutex::new(String::new()));
    let buf = Arc::clone(&stderr);
    let reader = std::thread::spawn(move || {
        use std::io::BufRead;
        let mut lines = std::io::BufReader::new(pipe);
        let mut line = String::new();
        loop {
            line.clear();
            match lines.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => buf.lock().unwrap().push_str(&line),
            }
        }
    });
    NodeProc {
        child: Some(child),
        stderr,
        reader: Some(reader),
    }
}

async fn stop_node(rpc: SocketAddr, mut node: NodeProc) {
    let _ = jsonrpc(rpc, "stop", json!([])).await;
    let mut child = node.child.take().expect("child");
    let status = tokio::task::spawn_blocking(move || child.wait())
        .await
        .expect("wait join")
        .unwrap_or_else(|e| panic!("wait node: {e}"));
    assert!(status.success(), "rbitcoin-node exit {status}");
}

fn child_of(prev: &Block, height: u32, time: u32, fault: impl FnOnce(&mut Block)) -> Block {
    let mut block = mine_empty_regtest(prev.block_hash(), time, height);
    fault(&mut block);
    prepare_regtest_candidate(&mut block, prev.block_hash(), time);
    block
}

/// The chain the trusted peer serves. Height 1's coinbase commits to height
/// 99 (`bad-cb-height`).
fn served_chain(now: u32) -> Vec<Block> {
    let mut chain = vec![regtest_genesis()];
    for height in 1..=3u32 {
        let time = now - 400 + height * 60;
        let block = child_of(chain.last().unwrap(), height, time, |b| {
            if height == 1 {
                b.txdata[0].input[0].script_sig =
                    ScriptBuf::from_bytes(rbitcoin_consensus::bip34_height_script(99));
            }
        });
        chain.push(block);
    }
    chain
}

/// A child of `tip` whose coinbase claims one satoshi above the subsidy
/// (`bad-cb-amount`).
fn excess_subsidy(tip: &Block, height: u32, time: u32) -> Block {
    child_of(tip, height, time, |b| {
        let value = b.txdata[0].output[0].value;
        b.txdata[0].output[0].value = value + Amount::from_sat(1);
    })
}

type Chain = Arc<Mutex<Vec<Block>>>;

/// One trusted listener. Every socket the node dials is served from the same
/// chain, and an announced block goes to every live socket.
struct TrustedPeer {
    addr: SocketAddr,
    chain: Chain,
    accepts: Arc<AtomicUsize>,
    live: Arc<AtomicUsize>,
    announce: broadcast::Sender<Header>,
}

impl TrustedPeer {
    fn announce(&self, block: &Block) {
        self.chain.lock().unwrap().push(block.clone());
        let _ = self.announce.send(block.header);
    }
}

async fn trusted_peer(chain: Vec<Block>) -> TrustedPeer {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listen");
    let peer = TrustedPeer {
        addr: listener.local_addr().expect("addr"),
        chain: Arc::new(Mutex::new(chain)),
        accepts: Arc::new(AtomicUsize::new(0)),
        live: Arc::new(AtomicUsize::new(0)),
        announce: broadcast::channel(8).0,
    };
    let addr = peer.addr;
    let chain = Arc::clone(&peer.chain);
    let accepts = Arc::clone(&peer.accepts);
    let live = Arc::clone(&peer.live);
    let announce = peer.announce.clone();
    tokio::spawn(async move {
        while let Ok((stream, remote)) = listener.accept().await {
            accepts.fetch_add(1, Ordering::SeqCst);
            live.fetch_add(1, Ordering::SeqCst);
            let chain = Arc::clone(&chain);
            let live = Arc::clone(&live);
            let headers = announce.subscribe();
            tokio::spawn(async move {
                let _ = serve(stream, addr, remote, chain, headers).await;
                live.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    peer
}

async fn serve(
    stream: tokio::net::TcpStream,
    addr: SocketAddr,
    remote: SocketAddr,
    chain: Chain,
    mut announced: broadcast::Receiver<Header>,
) -> Result<(), rbitcoin_net::NetError> {
    let start_height = i32::try_from(chain.lock().unwrap().len() - 1).unwrap();
    let (_ver, mut reader, mut writer, _wire, _tcp) = connect_and_handshake(
        stream,
        Magic::REGTEST,
        addr,
        remote,
        start_height,
        true,
        "/trusted-peer/",
        HandshakePolicy::plain(),
    )
    .await?;
    loop {
        let frame = tokio::select! {
            Ok(header) = announced.recv() => {
                write_v2_msg(&mut writer, NetworkMessage::Headers(vec![header])).await?;
                continue;
            }
            frame = read_v2_frame(&mut reader, Magic::REGTEST) => frame?,
        };
        let replies = match frame.decode().payload() {
            NetworkMessage::GetHeaders(request) => {
                let chain = chain.lock().unwrap();
                let start = chain
                    .iter()
                    .rposition(|b| request.locator_hashes.contains(&b.block_hash()))
                    .map_or(1, |height| height + 1);
                let headers = chain[start..].iter().map(|b| b.header).collect();
                vec![NetworkMessage::Headers(headers)]
            }
            NetworkMessage::GetData(items) => {
                let chain = chain.lock().unwrap();
                items
                    .iter()
                    .filter_map(|item| match item {
                        Inventory::Block(hash)
                        | Inventory::WitnessBlock(hash)
                        | Inventory::CompactBlock(hash) => {
                            chain.iter().find(|b| b.block_hash() == *hash)
                        }
                        _ => None,
                    })
                    .map(|b| NetworkMessage::Block(b.clone()))
                    .collect()
            }
            NetworkMessage::Ping(nonce) => vec![NetworkMessage::Pong(*nonce)],
            _ => continue,
        };
        for reply in replies {
            write_v2_msg(&mut writer, reply).await?;
        }
    }
}

async fn wait_tip(rpc: SocketAddr, want: BlockHash, node: &NodeProc, what: &str) {
    let deadline = Instant::now() + llvm_cov_wall(30, 90);
    let want = want.to_string();
    loop {
        let best = jsonrpc(rpc, "getbestblockhash", json!([])).await;
        if best["result"].as_str() == Some(want.as_str()) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{what}: tip {best} never reached {want}\n--- node log ---\n{}",
            node.log()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn applied_line(hash: BlockHash, height: u32) -> String {
    format!("trust-peer: applied block {hash} height {height}:")
}

/// The applied line can trail the tip on the stderr pipe.
async fn assert_applied(node: &NodeProc, block: &Block, height: u32, reason: &str) {
    let needle = applied_line(block.block_hash(), height);
    let deadline = Instant::now() + Duration::from_secs(5);
    let line = loop {
        let log = node.log();
        if let Some(line) = log.lines().find(|line| line.contains(&needle)) {
            break line.to_string();
        }
        assert!(
            Instant::now() < deadline,
            "no {needle:?} in the operator log:\n{log}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    assert!(
        line.contains("WARN"),
        "applied line is not a warning: {line}"
    );
    assert!(line.contains(reason), "applied line lost {reason}: {line}");
    let log = node.log();
    assert_eq!(
        log.matches(&needle).count(),
        1,
        "one fault, one line:\n{log}"
    );
}

async fn trust_peer_story() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    let chain = served_chain(now);
    let tip = chain.last().unwrap().clone();
    let peer = trusted_peer(chain.clone()).await;
    let home = TestDatadir::new().unwrap();
    let dir = home.path();
    write_token(&dir);
    let node = spawn_trusting_node(&dir, peer.addr);
    let rpc = rbitcoin_node::run::wait_published_listener(&dir, "rpc", llvm_cov_wall(30, 90)).await;

    // Catch-up from the trusted peer applies the BIP34 fault and warns once.
    wait_tip(rpc, tip.block_hash(), &node, "catch-up").await;
    assert_applied(&node, &chain[1], 1, "bad-cb-height").await;

    // A local submit of an excess-subsidy block stays strict.
    let excess = excess_subsidy(&tip, 4, now - 30);
    let submitted = jsonrpc(rpc, "submitblock", json!([serialize_hex(&excess)])).await;
    assert_eq!(submitted["result"], json!("bad-cb-amount"), "{submitted}");
    let best = jsonrpc(rpc, "getbestblockhash", json!([])).await;
    assert_eq!(best["result"], json!(tip.block_hash().to_string()));
    let log = node.log();
    assert!(
        !log.contains(&applied_line(excess.block_hash(), 4)),
        "local submit was waived:\n{log}"
    );

    // The same block from the trusted session is applied with a warning.
    peer.announce(&excess);
    wait_tip(rpc, excess.block_hash(), &node, "announce").await;
    assert_applied(&node, &excess, 4, "bad-cb-amount").await;

    // That session is not dropped or discouraged: its next block moves the
    // tip, with no new dial.
    let accepts = peer.accepts.load(Ordering::SeqCst);
    let clean = child_of(&excess, 5, now - 20, |_| {});
    peer.announce(&clean);
    wait_tip(rpc, clean.block_hash(), &node, "after the waiver").await;
    assert_eq!(
        peer.accepts.load(Ordering::SeqCst),
        accepts,
        "the node redialed"
    );
    assert_eq!(
        peer.live.load(Ordering::SeqCst),
        1,
        "trusted session dropped"
    );
    let peers = jsonrpc(rpc, "getpeerinfo", json!([])).await;
    let rows = peers["result"].as_array().expect("getpeerinfo rows");
    assert_eq!(rows.len(), 1, "{peers}");
    assert_eq!(rows[0]["addr"], json!(peer.addr.to_string()), "{peers}");

    stop_node(rpc, node).await;
}

/// One node started with `--trust-peer`, one trusted peer. Catch-up applies a
/// BIP34 fault with the documented warn line. A local `submitblock` of an
/// excess-subsidy block is rejected while the mode is on, and the same block
/// announced by the trusted session is applied. That session is neither
/// dropped nor discouraged.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trust_peer_applies_waivable_faults_from_its_session_only() {
    let wall = llvm_cov_wall(90, 240);
    tokio::time::timeout(wall, trust_peer_story())
        .await
        .unwrap_or_else(|_| {
            panic!("trust_peer_applies_waivable_faults_from_its_session_only wall")
        });
}
