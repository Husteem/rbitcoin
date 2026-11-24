use rbitcoin_net::eviction::*;

/// An ordinary inbound peer alone in its netgroup: relays txs, offers the
/// wanted services, has sent no block or tx, and pings slower than every
/// peer the ping protect keeps.
fn peer(id: u64, connected_at: u64) -> InboundEvictCandidate {
    InboundEvictCandidate {
        id,
        connected_at,
        min_ping: Some(1.0),
        netgroup: id,
        relay_txs: true,
        relevant_services: true,
        ..Default::default()
    }
}

/// Fillers newer than every subject, one cohort per fixed protect (Core
/// `SelectNodeToEvict`): 4 highest keyed netgroups, 8 lowest pings, 4 latest
/// txs, 8 non-tx-relay peers with the latest blocks, 4 latest blocks. Each
/// cohort is kept only by its own protect, so a missing protect leaves a
/// filler, the newest peer, as the victim.
fn fillers() -> Vec<InboundEvictCandidate> {
    let mut v = Vec::new();
    let mut id = 1_000;
    let mut next = |v: &mut Vec<InboundEvictCandidate>, f: &dyn Fn(&mut InboundEvictCandidate)| {
        id += 1;
        let mut c = peer(id, 1_000_000 + id);
        f(&mut c);
        v.push(c);
    };
    for i in 0..4 {
        next(&mut v, &|c| c.netgroup = u64::MAX - i);
    }
    for _ in 0..8 {
        next(&mut v, &|c| c.min_ping = Some(0.001));
    }
    for i in 0..4 {
        next(&mut v, &|c| c.last_tx = 10_000 + i);
    }
    for i in 0..8 {
        next(&mut v, &|c| {
            c.relay_txs = false;
            c.last_block = 10_000 + i;
        });
    }
    for i in 0..4 {
        next(&mut v, &|c| c.last_block = 20_000 + i);
    }
    v
}

/// The fillers and `n` subjects, ids 1..=n connected at 100..: none of the
/// fixed protects keeps a subject.
fn with_subjects(n: u64) -> Vec<InboundEvictCandidate> {
    let mut v = fillers();
    v.extend((1..=n).map(|i| peer(i, 99 + i)));
    v
}

#[test]
fn each_fixed_protect_keeps_its_cohort() {
    // Ten subjects: the five oldest are the uptime half, and the newest of
    // the rest, alone in its netgroup like every other, is the victim.
    assert_eq!(select_inbound_eviction(with_subjects(10)), Some(10));
}

#[test]
fn half_the_rest_is_kept_by_uptime() {
    // Thirty subjects. The fifteen oldest share one netgroup, the largest.
    // Half of thirty keeps all of them, so the newest single peer goes.
    // Keeping fewer leaves that group the largest, and its newest goes.
    let mut cands = with_subjects(30);
    for c in cands.iter_mut().filter(|c| (1..=15).contains(&c.id)) {
        c.netgroup = 7;
    }
    assert_eq!(select_inbound_eviction(cands), Some(30));
}

#[test]
fn a_tie_goes_to_the_group_with_the_youngest_member() {
    // Eight subjects: the four oldest are kept by uptime. The other four
    // are two groups of two. Group 99 holds the newest peer, so its newest
    // member goes, though group 1 has the lower key.
    let mut cands = with_subjects(8);
    for c in cands.iter_mut() {
        match c.id {
            5 | 7 => c.netgroup = 1,
            6 | 8 => c.netgroup = 99,
            _ => {}
        }
    }
    assert_eq!(select_inbound_eviction(cands), Some(8));
}

#[test]
fn onion_and_local_peers_share_the_uptime_protection() {
    // 24 subjects, so 12 are kept by uptime and up to 6 of those by network.
    // Three onion peers (group 50) and three local peers (group 51) are the
    // newest and each fill the largest groups. Three slots go to each
    // network, then the six oldest of the rest. The newest remaining
    // ordinary peer goes.
    let mut cands = with_subjects(24);
    for c in cands.iter_mut() {
        if (19..=21).contains(&c.id) {
            c.network = EvictNet::Onion;
            c.netgroup = 50;
            c.connected_at += 1_000;
        } else if (22..=24).contains(&c.id) {
            c.is_local = true;
            c.netgroup = 51;
            c.connected_at += 1_000;
        }
    }
    assert_eq!(select_inbound_eviction(cands), Some(18));
}

#[test]
fn block_relay_only_protect_needs_the_wanted_services() {
    // A non-tx-relay peer with a newer block than every block-relay filler
    // but without the wanted services takes a place in that window and is
    // not kept, so it, the newest peer, goes.
    let mut cands = with_subjects(10);
    let mut odd = peer(500, 5_000_000);
    odd.relay_txs = false;
    odd.relevant_services = false;
    odd.last_block = 15_000;
    cands.push(odd);
    assert_eq!(select_inbound_eviction(cands), Some(500));
}

#[test]
fn noban_never_evicted_alone() {
    let cands = vec![InboundEvictCandidate {
        noban: true,
        ..peer(7, 1)
    }];
    assert!(select_inbound_eviction(cands).is_none());
}

#[test]
fn maxconnections_shaped_protect_count() {
    // Fewer than the fixed protects → nothing to evict.
    let cands: Vec<_> = (0..8).map(|i| peer(i, i)).collect();
    assert!(select_inbound_eviction(cands).is_none());
}

#[test]
fn eviction_netgroup_ipv4_slash16() {
    let a: std::net::SocketAddr = "1.2.3.4:1".parse().unwrap();
    let b: std::net::SocketAddr = "1.2.9.9:1".parse().unwrap();
    let c: std::net::SocketAddr = "1.3.0.1:1".parse().unwrap();
    assert_eq!(eviction_netgroup(a), eviction_netgroup(b));
    assert_ne!(eviction_netgroup(a), eviction_netgroup(c));
    let mapped: std::net::SocketAddr = "[::ffff:1.2.3.4]:1".parse().unwrap();
    assert_eq!(
        eviction_netgroup(mapped),
        eviction_netgroup(b),
        "a dual-stack accept of 1.2.3.4 is still 1.2/16"
    );
}

#[test]
fn eviction_network_and_local() {
    use rbitcoin_net::NetAddr;
    let ip = |s: &str| NetAddr::Ip(s.parse().unwrap());
    let bind: std::net::SocketAddr = "127.0.0.1:8334".parse().unwrap();
    let other: std::net::SocketAddr = "0.0.0.0:8333".parse().unwrap();
    assert!(is_local_addr(&ip("127.0.0.1:1")));
    assert!(is_local_addr(&ip("[::1]:1")));
    assert!(is_local_addr(&ip("[::ffff:127.0.0.1]:1")));
    assert!(!is_local_addr(&ip("1.2.3.4:1")));
    assert_eq!(
        evict_net(&ip("127.0.0.1:1"), bind, Some(bind)),
        EvictNet::Onion,
        "the onion listener is onion"
    );
    assert_eq!(
        evict_net(&ip("127.0.0.1:1"), other, Some(bind)),
        EvictNet::Other
    );
    assert_eq!(evict_net(&ip("[fc00::1]:1"), other, None), EvictNet::Cjdns);
}

#[test]
fn the_netgroup_key_is_per_process() {
    use std::hash::BuildHasher;
    let a = rbitcoin_net::PeerHub::new();
    let b = rbitcoin_net::PeerHub::new();
    let order = |hub: &rbitcoin_net::PeerHub| {
        let mut g: Vec<u64> = (0..64).collect();
        g.sort_by_key(|n| hub.netgroup_key.hash_one(n));
        g
    };
    assert_ne!(order(&a), order(&b), "each hub draws its own key");
    assert_ne!(order(&a), (0..64).collect::<Vec<_>>(), "not the raw order");
}
