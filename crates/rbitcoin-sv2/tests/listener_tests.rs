use crate::test_chain::{shared_regtest, test_tp_config};
use common_messages_sv2::{
    SetupConnectionError, SetupConnectionSuccess, MESSAGE_TYPE_SETUP_CONNECTION_ERROR,
    MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
};
use rbitcoin_sv2::testutil::TpClient;
use rbitcoin_sv2::{
    run_sv2_tp, MAX_IDLE_TIMEOUT, MAX_SESSIONS, MAX_SESSIONS_PER_NETGROUP, MAX_STALE_GRACE,
    MAX_TEMPLATE_INTERVAL, MIN_TEMPLATE_INTERVAL, TEMPLATE_INTERVAL,
};
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use std::time::Instant;
use template_distribution_sv2::MESSAGE_TYPE_REQUEST_TRANSACTION_DATA_ERROR;

const TDP: u8 = 2;

async fn connect_when_free(addr: SocketAddr, pk: [u8; 32]) -> TpClient {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match TpClient::connect(addr, pk).await {
            Ok(c) => return c,
            Err(e) if Instant::now() > deadline => panic!("no free session slot: {e}"),
            Err(_) => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
}

async fn expect_error(c: &mut TpClient, flags: u32, code: &str) {
    let mut f = c.recv().await.expect("setup reply");
    assert_eq!(f.msg_type, MESSAGE_TYPE_SETUP_CONNECTION_ERROR);
    let e: SetupConnectionError = binary_sv2::from_bytes(&mut f.payload).expect("decode");
    assert_eq!(e.flags, flags);
    assert_eq!(e.error_code.as_utf8_or_hex(), code);
    let closed = tokio::time::timeout(Duration::from_secs(5), c.recv()).await;
    assert!(
        matches!(closed, Ok(Err(_))),
        "connection must close after SetupConnection.Error"
    );
}

/// The listener on every address, and one source address in each of two
/// netgroups: loopback IPv4 (`127/16`) and loopback IPv6 (`::/32`).
fn dual_stack_sources(listen: SocketAddr) -> [SocketAddr; 2] {
    let v4 = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), listen.port());
    let v6 = SocketAddr::new(Ipv6Addr::LOCALHOST.into(), listen.port());
    [v4, v6]
}

async fn setup_ok(addr: SocketAddr, pk: [u8; 32]) -> TpClient {
    let mut c = TpClient::connect(addr, pk).await.expect("handshake");
    c.setup_connection(TDP, 2, 2, 0).await.unwrap();
    let mut f = c.recv().await.expect("setup reply");
    assert_eq!(f.msg_type, MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS);
    let ok: SetupConnectionSuccess = binary_sv2::from_bytes(&mut f.payload).expect("decode");
    assert_eq!((ok.used_version, ok.flags), (2, 0));
    c
}

/// `netgroup_sessions` replaces the default share: miners behind one NAT,
/// one LAN or one loopback tunnel can take more of the table, or the
/// operator can tighten it. The global cap still applies.
#[tokio::test]
async fn netgroup_sessions_sets_the_share() {
    let tc = shared_regtest(0);
    for share in [1, MAX_SESSIONS] {
        let tp = run_sv2_tp({
            let mut cfg = test_tp_config(Arc::clone(&tc.chain), [7; 32]);
            cfg.netgroup_sessions = share;
            cfg
        })
        .await
        .expect("listen");
        let (addr, pk) = (tp.local_addr, tp.authority_pubkey);
        let mut live = Vec::new();
        for _ in 0..share {
            live.push(setup_ok(addr, pk).await);
        }
        assert!(
            TpClient::connect(addr, pk).await.is_err(),
            "loopback past a share of {share} must close before the handshake"
        );
        drop(live);
        tp.shutdown().await;
    }
}

/// An accepted session's socket has TCP keepalive on (and, on Linux, a TCP
/// user timeout), so a client that drops without a FIN frees its slot and
/// netgroup share in about a minute, not after the kernel's retransmit
/// timeout.
#[test]
fn accepted_sockets_get_tcp_keepalive() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let _client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (accepted, _) = listener.accept().unwrap();
    accepted.set_nonblocking(true).unwrap();
    let accepted = {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();
        let _g = rt.enter();
        let s = tokio::net::TcpStream::from_std(accepted).unwrap();
        rbitcoin_sv2::set_session_keepalive(&s).unwrap();
        s.into_std().unwrap()
    };
    let sock = socket2::SockRef::from(&accepted);
    assert!(sock.keepalive().unwrap(), "SO_KEEPALIVE");
    #[cfg(target_os = "linux")]
    {
        assert_eq!(sock.tcp_keepalive_time().unwrap(), Duration::from_secs(30));
        assert_eq!(
            sock.tcp_keepalive_interval().unwrap(),
            Duration::from_secs(10)
        );
        assert_eq!(sock.tcp_keepalive_retries().unwrap(), 3);
        assert_eq!(
            sock.tcp_user_timeout().unwrap(),
            Some(Duration::from_secs(60))
        );
    }
}

/// A share outside 1..=MAX_SESSIONS is refused at start, as the other
/// bounded fields are.
#[tokio::test]
async fn netgroup_sessions_out_of_range_is_refused() {
    let tc = shared_regtest(0);
    for share in [0, MAX_SESSIONS + 1] {
        let mut cfg = test_tp_config(Arc::clone(&tc.chain), [7; 32]);
        cfg.netgroup_sessions = share;
        let err = run_sv2_tp(cfg).await.err().expect("refused");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");
    }
}

/// One netgroup takes at most half the table, so idle clients from one
/// network cannot lock out a miner from another. The newcomer is refused;
/// existing sessions are not touched.
#[tokio::test]
async fn setup_connection_success_errors_and_session_cap() {
    let tc = shared_regtest(0);
    let chain = Arc::clone(&tc.chain);
    let tp = run_sv2_tp({
        let mut cfg = test_tp_config(chain, [7; 32]);
        cfg.listen = "[::]:0".parse().unwrap();
        cfg
    })
    .await
    .expect("listen");
    let pk = tp.authority_pubkey;
    let [v4, v6] = dual_stack_sources(tp.local_addr);
    let share = MAX_SESSIONS_PER_NETGROUP;

    let mut live = Vec::new();
    for _ in 0..share {
        live.push(setup_ok(v4, pk).await);
    }
    assert!(
        TpClient::connect(v4, pk).await.is_err(),
        "one netgroup over its share must close before the handshake"
    );
    for _ in 0..MAX_SESSIONS - share {
        live.push(setup_ok(v6, pk).await);
    }
    // The global cap is checked first, so this refusal is the cap's own,
    // not the IPv6 netgroup's share.
    rbitcoin_log::reset_repeated_info();
    rbitcoin_log::capture_logs(true);
    let _ = rbitcoin_log::take_logs();
    assert!(
        TpClient::connect(v6, pk).await.is_err(),
        "session over the cap must close before the handshake"
    );
    let lines = rbitcoin_log::take_logs();
    rbitcoin_log::capture_logs(false);
    assert!(
        lines.iter().any(|(_, m)| m.contains("at max_sessions=")),
        "the global cap refused it: {lines:?}"
    );
    for c in live.iter_mut().step_by(MAX_SESSIONS - 1) {
        let still_open = tokio::time::timeout(Duration::from_millis(200), c.recv()).await;
        assert!(
            still_open.is_err(),
            "existing session must stay up at the cap"
        );
    }
    // A closed session returns its netgroup's share.
    live.truncate(MAX_SESSIONS - 1);
    drop(connect_when_free(v6, pk).await);
    drop(live);

    let mut c = connect_when_free(v4, pk).await;
    c.setup_connection(TDP, 2, 2, 0b101).await.unwrap();
    expect_error(&mut c, 0b101, "unsupported-feature-flags").await;

    let mut c = connect_when_free(v4, pk).await;
    c.setup_connection(0, 2, 2, 0).await.unwrap();
    expect_error(&mut c, 0, "unsupported-protocol").await;

    let mut c = connect_when_free(v4, pk).await;
    c.setup_connection(TDP, 3, 4, 0).await.unwrap();
    expect_error(&mut c, 0, "protocol-version-mismatch").await;

    tp.shutdown().await;
}

/// key-utils 1.2.0 vector: SRI clients configure the TP authority key in
/// this form, so the handle must print it, and a client must connect with it.
#[tokio::test]
async fn authority_key_prints_in_key_utils_base58check() {
    let tc = shared_regtest(0);
    let chain = Arc::clone(&tc.chain);
    let secret = bitcoin::base58::decode_check("zmBEmPhqo3A92FkiLVvyCz6htc3e53ph3ZbD4ASqGaLjwnFLi")
        .expect("vector secret");
    let tp = run_sv2_tp(test_tp_config(
        chain,
        secret.try_into().expect("32-byte secret"),
    ))
    .await
    .expect("listen");
    let key = tp.authority_key();
    assert_eq!(key, "9bDuixKmZqAJnrmP746n8zU1wyAQRrus7th9dxnkPg6RzQvCnan");

    let decoded = bitcoin::base58::decode_check(&key).unwrap();
    let pk: [u8; 32] = decoded[2..].try_into().unwrap();
    let mut c = TpClient::connect(tp.local_addr, pk)
        .await
        .expect("handshake against the printed key");
    c.setup_connection(TDP, 2, 2, 0).await.unwrap();
    let mut f = c.recv().await.expect("setup reply");
    assert_eq!(f.msg_type, MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS);
    let _: SetupConnectionSuccess = binary_sv2::from_bytes(&mut f.payload).expect("decode");
    tp.shutdown().await;
}

/// Silent sockets from two netgroups take every slot at accept; the setup
/// deadline must close them so a real client gets in.
#[tokio::test]
async fn silent_sockets_are_dropped_at_the_setup_deadline() {
    let tc = shared_regtest(0);
    let chain = Arc::clone(&tc.chain);
    use tokio::io::AsyncReadExt;

    let setup_timeout = Duration::from_millis(300);
    let tp = run_sv2_tp({
        let mut cfg = test_tp_config(chain, [7; 32]);
        cfg.listen = "[::]:0".parse().unwrap();
        cfg.setup_timeout = setup_timeout;
        cfg
    })
    .await
    .expect("listen");
    let pk = tp.authority_pubkey;
    let [addr, v6] = dual_stack_sources(tp.local_addr);

    let mut silent = Vec::new();
    for to in [addr, v6].into_iter().cycle().take(MAX_SESSIONS) {
        silent.push(tokio::net::TcpStream::connect(to).await.expect("tcp"));
    }
    for s in &mut silent {
        let mut b = [0u8; 1];
        let closed = tokio::time::timeout(setup_timeout * 10, s.read(&mut b)).await;
        assert!(
            matches!(closed, Ok(Ok(0) | Err(_))),
            "silent socket must be closed after the setup deadline"
        );
    }

    let mut c = connect_when_free(addr, pk).await;
    c.setup_connection(TDP, 2, 2, 0).await.unwrap();
    let f = c.recv().await.expect("setup reply");
    assert_eq!(f.msg_type, MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS);
    tp.shutdown().await;
}

/// A TDP session without `CoinbaseOutputConstraints` never gets a template
/// and never writes, so the setup deadline also covers the first constraints.
/// Other frames before them do not reset it.
#[tokio::test]
async fn session_without_constraints_is_dropped_at_the_setup_deadline() {
    let tc = shared_regtest(0);
    let chain = Arc::clone(&tc.chain);
    let setup_timeout = Duration::from_millis(300);
    let tp = run_sv2_tp({
        let mut cfg = test_tp_config(chain, [7; 32]);
        cfg.setup_timeout = setup_timeout;
        cfg
    })
    .await
    .expect("listen");
    let (addr, pk) = (tp.local_addr, tp.authority_pubkey);

    let mut idle = TpClient::connect(addr, pk).await.expect("handshake");
    idle.setup_connection(TDP, 2, 2, 0).await.unwrap();
    idle.recv().await.expect("setup reply");
    idle.request_transaction_data(1).await.unwrap();
    let f = idle
        .recv()
        .await
        .expect("request answered before constraints");
    assert_eq!(f.msg_type, MESSAGE_TYPE_REQUEST_TRANSACTION_DATA_ERROR);

    let mut ok = TpClient::connect(addr, pk).await.expect("handshake");
    ok.setup_connection(TDP, 2, 2, 0).await.unwrap();
    ok.recv().await.expect("setup reply");
    ok.coinbase_output_constraints(1, 1).await.unwrap();

    // Same-budget constraints do not rebuild. They do refresh the idle
    // deadline, so this session can outlive the original setup deadline.
    let since_constraints = Instant::now();
    let closed = tokio::time::timeout(setup_timeout * 10, async {
        loop {
            tokio::select! {
                biased;
                r = idle.recv() => {
                    if r.is_err() {
                        break;
                    }
                }
                _ = tokio::time::sleep(setup_timeout / 3) => {
                    ok.coinbase_output_constraints(1, 1)
                        .await
                        .expect("same budget keeps the session");
                }
            }
        }
    })
    .await;
    assert!(closed.is_ok(), "session without constraints must be closed");

    while since_constraints.elapsed() < setup_timeout * 2 {
        tokio::time::sleep(setup_timeout / 3).await;
        ok.coinbase_output_constraints(1, 1)
            .await
            .expect("same budget keeps the session");
    }
    ok.request_transaction_data(u64::MAX).await.unwrap();
    loop {
        let f = ok
            .recv()
            .await
            .expect("session with constraints stays open");
        if f.msg_type == MESSAGE_TYPE_REQUEST_TRANSACTION_DATA_ERROR {
            break;
        }
    }
    tp.shutdown().await;
}

/// One `CoinbaseOutputConstraints` then silence. The setup deadline has
/// already passed, so only the opt-in idle deadline frees the slot.
#[tokio::test]
async fn session_silent_after_constraints_is_dropped_at_the_idle_deadline() {
    let tc = shared_regtest(0);
    let chain = Arc::clone(&tc.chain);
    let setup_timeout = Duration::from_millis(300);
    let tp = run_sv2_tp({
        let mut cfg = test_tp_config(chain, [7; 32]);
        cfg.setup_timeout = setup_timeout;
        cfg.idle_timeout = Some(setup_timeout);
        cfg
    })
    .await
    .expect("listen");
    let (addr, pk) = (tp.local_addr, tp.authority_pubkey);

    let mut c = TpClient::connect(addr, pk).await.expect("handshake");
    c.setup_connection(TDP, 2, 2, 0).await.unwrap();
    c.recv().await.expect("setup reply");
    c.coinbase_output_constraints(1, 1).await.unwrap();

    let closed = tokio::time::timeout(setup_timeout * 10, async {
        while c.recv().await.is_ok() {}
    })
    .await;
    assert!(
        closed.is_ok(),
        "session silent after constraints must be closed"
    );
    tp.shutdown().await;
}

/// A client that floods requests and never reads jams the TP's writes; the
/// write deadline must close the session instead of stalling it forever.
#[tokio::test]
async fn client_that_stops_reading_is_dropped_at_the_write_deadline() {
    let tc = shared_regtest(0);
    let chain = Arc::clone(&tc.chain);
    let write_timeout = Duration::from_millis(200);
    let tp = run_sv2_tp({
        let mut cfg = test_tp_config(chain, [7; 32]);
        cfg.write_timeout = write_timeout;
        cfg
    })
    .await
    .expect("listen");
    // This listener only. Drop removes the entry.
    #[cfg(target_os = "linux")]
    let _send_buf = rbitcoin_sv2::test_send_buffer::pin(tp.local_addr, 8 * 1024);
    // Pin the receive window too. A small SO_SNDBUF still drains while this
    // socket ACKs into an autotuned window, so write() never stalls.
    let mut c = TpClient::connect_recv_buffer(tp.local_addr, tp.authority_pubkey, Some(2048))
        .await
        .expect("handshake");
    c.setup_connection(TDP, 2, 2, 0).await.unwrap();
    c.recv().await.expect("setup reply");
    // Without constraints the setup deadline closes the session at
    // SETUP_TIMEOUT: the flood must lose only to the write deadline. The
    // stale tip holds the template, so constraints add no traffic.
    c.coinbase_output_constraints(0, 0).await.unwrap();

    // Each unknown id answers RequestTransactionData.Error. This client never
    // reads those frames. Keep sending until the stalled write closes the
    // socket: a cancelled send is not a full window, and stopping there
    // leaves the session idle so the deadline never starts. Yield so the
    // session task runs; a ready send does not, and the flood only fills
    // the kernel buffer.
    let started = Instant::now();
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        for id in 1..=1_000_000u64 {
            c.request_transaction_data(id).await?;
            tokio::task::yield_now().await;
        }
        Ok::<(), io::Error>(())
    })
    .await;
    match outcome {
        Ok(Err(e)) if e.kind() != io::ErrorKind::TimedOut => {}
        Ok(Err(e)) => panic!("client write timed out; session stayed open: {e}"),
        Ok(Ok(())) => panic!("socket buffers never filled"),
        Err(_) => panic!("session must close once its write stalls past the deadline"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "stalled write took {:?}",
        started.elapsed()
    );
    tp.shutdown().await;
}

/// The largest legitimate client frame (a `SubmitSolution` with a full
/// `B064K` coinbase) keeps the session; a larger frame closes it.
#[tokio::test]
async fn oversized_client_frame_closes_the_session() {
    let tc = shared_regtest(0);
    let chain = Arc::clone(&tc.chain);
    let tp = run_sv2_tp(test_tp_config(chain, [7; 32]))
        .await
        .expect("listen");
    let mut c = TpClient::connect(tp.local_addr, tp.authority_pubkey)
        .await
        .expect("handshake");
    c.setup_connection(TDP, 2, 2, 0).await.unwrap();
    c.recv().await.expect("setup reply");

    let coinbase = vec![0u8; usize::from(u16::MAX)];
    c.submit_solution(1, 0, 0, 0, &coinbase).await.unwrap();
    c.request_transaction_data(1).await.unwrap();
    let f = c
        .recv()
        .await
        .expect("open after a max-size SubmitSolution");
    assert_eq!(f.msg_type, MESSAGE_TYPE_REQUEST_TRANSACTION_DATA_ERROR);

    let _ = c.send_bytes(0xff, &vec![0u8; 1 << 20]).await;
    c.request_transaction_data(1).await.ok();
    let closed = tokio::time::timeout(Duration::from_secs(5), c.recv()).await;
    assert!(
        matches!(closed, Ok(Err(_))),
        "an oversized frame must close the session, got {:?}",
        closed.map(|r| r.map(|f| f.msg_type))
    );
    tp.shutdown().await;
}

#[tokio::test]
async fn out_of_range_timing_refuses_to_start() {
    let tc = shared_regtest(0);
    let chain = Arc::clone(&tc.chain);
    let hour = Duration::from_secs(3600);
    let over = Duration::from_secs(1);
    for (cert_validity, stale_grace, template_interval) in [
        (
            Duration::from_secs(u64::from(u32::MAX) + 1),
            Duration::ZERO,
            TEMPLATE_INTERVAL,
        ),
        (hour, MAX_STALE_GRACE + over, TEMPLATE_INTERVAL),
        (hour, Duration::ZERO, Duration::ZERO),
        (
            hour,
            Duration::ZERO,
            MIN_TEMPLATE_INTERVAL - Duration::from_millis(1),
        ),
        (hour, Duration::ZERO, MAX_TEMPLATE_INTERVAL + over),
    ] {
        let e = run_sv2_tp({
            let mut cfg = test_tp_config(Arc::clone(&chain), [7; 32]);
            cfg.cert_validity = cert_validity;
            cfg.stale_grace = stale_grace;
            cfg.template_interval = template_interval;
            cfg
        })
        .await
        .err()
        .expect("out-of-range config must not start");
        assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{e}");
    }
    let e = run_sv2_tp({
        let mut cfg = test_tp_config(chain, [7; 32]);
        cfg.cert_validity = hour;
        cfg.stale_grace = Duration::ZERO;
        cfg.idle_timeout = Some(MAX_IDLE_TIMEOUT + over);
        cfg
    })
    .await
    .err()
    .expect("out-of-range idle timeout must not start");
    assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{e}");
}

/// Any client can open connections, so connect and disconnect log at DEBUG.
/// At INFO they let an unauthenticated client fill the operator's log.
#[tokio::test]
async fn connect_and_disconnect_log_below_info() {
    use rbitcoin_log::Level;
    let tc = shared_regtest(0);
    let chain = Arc::clone(&tc.chain);
    rbitcoin_log::capture_logs(true);
    let tp = run_sv2_tp(test_tp_config(chain, [7; 32]))
        .await
        .expect("listen");
    let mut c = TpClient::connect(tp.local_addr, tp.authority_pubkey)
        .await
        .expect("handshake");
    c.setup_connection(TDP, 2, 2, 0).await.unwrap();
    let f = c.recv().await.expect("setup reply");
    assert_eq!(f.msg_type, MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS);
    drop(c);
    let mut logs = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !logs
        .iter()
        .any(|(_, m): &(Level, String)| m.starts_with("sv2: disconnect"))
    {
        assert!(Instant::now() < deadline, "no disconnect line: {logs:?}");
        tokio::time::sleep(Duration::from_millis(10)).await;
        logs.extend(rbitcoin_log::take_logs());
    }
    rbitcoin_log::capture_logs(false);
    tp.shutdown().await;
    let lines: Vec<_> = logs
        .iter()
        .filter(|(_, m)| m.starts_with("sv2: connect") || m.starts_with("sv2: disconnect"))
        .collect();
    assert_eq!(lines.len(), 2, "{logs:?}");
    assert!(lines.iter().all(|(l, _)| *l == Level::Debug), "{lines:?}");
}

/// A client causes any socket error by closing or resetting, so that end logs
/// at DEBUG. A session task that panicked is this side's fault and stays at WARN.
#[tokio::test]
async fn panicked_sv2_disconnect_logs_at_warn() {
    use rbitcoin_log::Level;
    let peer: SocketAddr = "127.0.0.1:1".parse().unwrap();
    let joined = tokio::spawn(async { panic!("handler bug") })
        .await
        .expect_err("the task panicked");
    rbitcoin_log::capture_logs(true);
    rbitcoin_sv2::log_disconnect(peer, &Ok(()));
    rbitcoin_sv2::log_disconnect(peer, &Err(std::io::ErrorKind::UnexpectedEof.into()));
    rbitcoin_sv2::log_disconnect(peer, &Err(std::io::Error::other(joined)));
    let levels: Vec<Level> = rbitcoin_log::take_logs()
        .into_iter()
        .map(|(l, _)| l)
        .collect();
    rbitcoin_log::capture_logs(false);
    assert_eq!(levels, [Level::Debug, Level::Debug, Level::Warn]);
}

/// A client that keeps connecting past its netgroup's share must not append
/// one log line per refused socket: the port has no client auth. The accept
/// loop runs on this thread, so the capture sees its lines.
#[tokio::test(flavor = "current_thread")]
async fn rejects_past_the_share_log_fewer_lines_than_rejects() {
    const EXTRA: usize = 20;
    let tc = shared_regtest(0);
    let chain = Arc::clone(&tc.chain);
    rbitcoin_log::reset_repeated_info();
    rbitcoin_log::capture_logs(true);
    let _ = rbitcoin_log::take_logs();
    let tp = run_sv2_tp(test_tp_config(chain, [7; 32]))
        .await
        .expect("listen");
    let mut opened = Vec::new();
    for _ in 0..MAX_SESSIONS_PER_NETGROUP + EXTRA {
        opened.push(
            tokio::net::TcpStream::connect(tp.local_addr)
                .await
                .expect("tcp"),
        );
        tokio::task::yield_now().await;
    }
    // Each refused socket reads EOF once the listener closed it, so every
    // refusal was logged (or suppressed) before the capture is read.
    {
        use tokio::io::AsyncReadExt;
        for s in &mut opened[MAX_SESSIONS_PER_NETGROUP..] {
            let mut b = [0u8; 1];
            let closed = tokio::time::timeout(Duration::from_secs(5), s.read(&mut b)).await;
            assert!(
                matches!(closed, Ok(Ok(0) | Err(_))),
                "a socket past the share was not refused"
            );
        }
    }
    let lines = rbitcoin_log::take_logs();
    rbitcoin_log::capture_logs(false);
    let rejects = lines
        .iter()
        .filter(|(_, msg)| msg.contains("sv2: reject"))
        .count();
    assert!(
        (1..EXTRA).contains(&rejects),
        "{rejects} reject lines for {EXTRA} refused sockets: {lines:?}"
    );
    drop(opened);
    tp.shutdown().await;
}
