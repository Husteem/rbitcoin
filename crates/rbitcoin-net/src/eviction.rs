//! Inbound peer eviction (Core `SelectNodeToEvict` / `AttemptToEvictConnection`).
//!
//! When inbound slots are full, accept a new peer only after disconnecting one
//! unprotected inbound. Protection follows Core: keyed netgroup, lowest
//! min-ping, recent txs, block-relay-only peers, recent blocks, then half of
//! the rest by uptime with a share for onion, local, I2P and CJDNS peers.

/// One inbound session considered for eviction (Core `NodeEvictionCandidate`).
#[derive(Clone, Debug, Default)]
pub struct InboundEvictCandidate {
    pub id: u64,
    pub connected_at: u64,
    pub min_ping: Option<f64>,
    pub last_block: u64,
    pub last_tx: u64,
    /// The peer's netgroup hashed with this process's secret key (Core
    /// `nKeyedNetGroup`), so a peer cannot pick its place in that order.
    pub netgroup: u64,
    pub noban: bool,
    /// The peer asked for transaction relay.
    pub relay_txs: bool,
    /// The peer offered every service this node wanted at its handshake.
    pub relevant_services: bool,
    /// Loopback address (Core `IsLocal`).
    pub is_local: bool,
    /// The network the peer reached this node through.
    pub network: EvictNet,
}

/// Networks whose peers Core reserves part of the uptime protection for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EvictNet {
    #[default]
    Other,
    Onion,
    I2p,
    Cjdns,
}

use std::cmp::Ordering;

type Cand = InboundEvictCandidate;

/// Pick one inbound id to disconnect, or `None` if every candidate is
/// protected. Core `SelectNodeToEvict`, without the bloom-filter tie-break
/// and `prefer_evict` (this node serves no bloom filters and refuses a
/// discouraged inbound).
pub fn select_inbound_eviction(mut cands: Vec<Cand>) -> Option<u64> {
    cands.retain(|c| !c.noban);
    // 4 peers by keyed netgroup: a peer cannot pick which groups stay.
    erase_last_k(&mut cands, |a, b| a.netgroup.cmp(&b.netgroup), 4, |_| true);
    // The 8 lowest minimum pings.
    erase_last_k(&mut cands, reverse_min_ping, 8, |_| true);
    // The 4 that last sent a tx the pool took.
    erase_last_k(&mut cands, tx_time, 4, |_| true);
    // Up to 8 non-tx-relay peers with the wanted services that sent blocks.
    erase_last_k(&mut cands, block_relay_only_time, 8, |c| {
        !c.relay_txs && c.relevant_services
    });
    // The 4 that last sent a new block.
    erase_last_k(&mut cands, block_time, 4, |_| true);
    protect_by_ratio(&mut cands);
    // `cands` is newest first now. The largest netgroup, a tie to the
    // group whose newest member is newer; its newest member goes.
    let mut groups: Vec<(u64, usize, u64, u64)> = Vec::new();
    let mut best: Option<usize> = None;
    for c in &cands {
        let i = match groups.iter().position(|g| g.0 == c.netgroup) {
            Some(i) => {
                groups[i].1 += 1;
                i
            }
            None => {
                groups.push((c.netgroup, 1, c.connected_at, c.id));
                groups.len() - 1
            }
        };
        let (_, n, newest, _) = groups[i];
        let wins = best.is_none_or(|b| {
            let (_, bn, bnewest, _) = groups[b];
            n > bn || (n == bn && newest > bnewest)
        });
        if wins {
            best = Some(i);
        }
    }
    best.map(|b| groups[b].3)
}

/// Sort by `cmp` (front: evict first), then drop each of the last `k` for
/// which `keep` holds. Equal keys fall back to the newer peer first, then
/// the higher id, where Core leaves the order to its sort.
fn erase_last_k(
    cands: &mut Vec<Cand>,
    cmp: impl Fn(&Cand, &Cand) -> Ordering,
    k: usize,
    keep: impl Fn(&Cand) -> bool,
) {
    cands.sort_by(|a, b| {
        cmp(a, b)
            .then_with(|| b.connected_at.cmp(&a.connected_at))
            .then_with(|| b.id.cmp(&a.id))
    });
    let start = cands.len().saturating_sub(k);
    let tail: Vec<Cand> = cands.drain(start..).filter(|c| !keep(c)).collect();
    cands.extend(tail);
}

/// Core `ReverseCompareNodeMinPingTime`: highest ping first; none is the
/// highest.
fn reverse_min_ping(a: &Cand, b: &Cand) -> Ordering {
    let p = |c: &Cand| c.min_ping.unwrap_or(f64::MAX);
    p(b).total_cmp(&p(a))
}

/// Core `ReverseCompareNodeTimeConnected`: newest first.
fn reverse_connected(a: &Cand, b: &Cand) -> Ordering {
    b.connected_at.cmp(&a.connected_at)
}

/// Core `CompareNodeTXTime` without the bloom-filter step.
fn tx_time(a: &Cand, b: &Cand) -> Ordering {
    a.last_tx
        .cmp(&b.last_tx)
        .then_with(|| a.relay_txs.cmp(&b.relay_txs))
        .then_with(|| reverse_connected(a, b))
}

/// Core `CompareNodeBlockRelayOnlyTime`: tx-relay peers first.
fn block_relay_only_time(a: &Cand, b: &Cand) -> Ordering {
    b.relay_txs
        .cmp(&a.relay_txs)
        .then_with(|| a.last_block.cmp(&b.last_block))
        .then_with(|| a.relevant_services.cmp(&b.relevant_services))
        .then_with(|| reverse_connected(a, b))
}

/// Core `CompareNodeBlockTime`.
fn block_time(a: &Cand, b: &Cand) -> Ordering {
    a.last_block
        .cmp(&b.last_block)
        .then_with(|| a.relevant_services.cmp(&b.relevant_services))
        .then_with(|| reverse_connected(a, b))
}

/// The networks Core reserves uptime protection for, in its order.
#[derive(Clone, Copy)]
enum Reserved {
    Net(EvictNet),
    Local,
}

impl Reserved {
    fn holds(self, c: &Cand) -> bool {
        match self {
            Reserved::Net(n) => c.network == n,
            Reserved::Local => c.is_local,
        }
    }
}

/// Core `ProtectEvictionCandidatesByRatio`: keep half of the rest by
/// uptime, up to half of those for onion, local, I2P and CJDNS peers, the
/// networks with fewer peers first.
fn protect_by_ratio(cands: &mut Vec<Cand>) {
    let total = cands.len() / 2;
    let mut nets: Vec<(Reserved, usize)> = [
        Reserved::Net(EvictNet::Cjdns),
        Reserved::Net(EvictNet::I2p),
        Reserved::Local,
        Reserved::Net(EvictNet::Onion),
    ]
    .into_iter()
    .map(|r| (r, cands.iter().filter(|c| r.holds(c)).count()))
    .collect();
    nets.sort_by_key(|n| n.1);
    let by_network = total / 2;
    let mut protected = 0usize;
    'rounds: while protected < by_network {
        let live = nets.iter().filter(|n| n.1 > 0).count();
        if live == 0 {
            break;
        }
        let each = ((by_network - protected) / live).max(1);
        let mut any = false;
        for (net, count) in nets.iter_mut() {
            if *count == 0 {
                continue;
            }
            let net = *net;
            let before = cands.len();
            erase_last_k(
                cands,
                |a, b| {
                    net.holds(a)
                        .cmp(&net.holds(b))
                        .then_with(|| reverse_connected(a, b))
                },
                each,
                |c| net.holds(c),
            );
            let gone = before - cands.len();
            if gone > 0 {
                any = true;
                protected += gone;
                if protected >= by_network {
                    break 'rounds;
                }
                *count -= gone;
            }
        }
        if !any {
            break;
        }
    }
    erase_last_k(cands, reverse_connected, total - protected, |_| true);
}

/// Core `CNetAddr::IsLocal`: IPv4 `127.0.0.0/8` or `0.0.0.0/8`, IPv6 `::1`.
pub fn is_local_addr(net: &crate::NetAddr) -> bool {
    match net {
        crate::NetAddr::Ip(a) => match a.ip().to_canonical() {
            std::net::IpAddr::V4(v4) => v4.octets()[0] == 127 || v4.octets()[0] == 0,
            std::net::IpAddr::V6(v6) => v6.is_loopback(),
        },
        _ => false,
    }
}

/// Core `ConnectedThroughNetwork` for an inbound peer: the onion listener
/// is onion whatever address the proxy forwards from.
pub fn evict_net(
    net: &crate::NetAddr,
    addrbind: std::net::SocketAddr,
    onion_bind: Option<std::net::SocketAddr>,
) -> EvictNet {
    if onion_bind == Some(addrbind) {
        return EvictNet::Onion;
    }
    match net {
        crate::NetAddr::Onion { .. } => EvictNet::Onion,
        crate::NetAddr::I2p { .. } => EvictNet::I2p,
        crate::NetAddr::Cjdns { .. } => EvictNet::Cjdns,
        crate::NetAddr::Ip(a) => match a.ip() {
            std::net::IpAddr::V6(v6) if crate::netaddr::is_cjdns_ip(v6) => EvictNet::Cjdns,
            _ => EvictNet::Other,
        },
        crate::NetAddr::Name { .. } => EvictNet::Other,
    }
}

/// Stable netgroup key for eviction: [`crate::netgroup::netgroup`] without asmap.
pub fn eviction_netgroup(addr: std::net::SocketAddr) -> u64 {
    crate::netgroup::netgroup(addr, None)
}
