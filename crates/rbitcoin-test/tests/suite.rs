rbitcoin_store::skip_flushes_before_main!();

mod asmap_journey;
mod consensus_rules;
mod core_analogs;
mod cross_surface;
mod electrum_protocol;
mod end_of_ibd;
mod i2p_dest;
mod integration_multinode;
mod jsonrpc;
mod mine;
mod outbound_refill_journey;
mod scenarios;
mod trust_peer_waiver_journey;

/// Admit as an unattributed relay through
/// [`rbitcoin_net::MempoolHub::accept_with_utxo`], the sync core that peer
/// relay and orphan promotion ship.
pub(crate) trait Admit {
    fn admit(
        &self,
        tx: &bitcoin::Transaction,
    ) -> Result<rbitcoin_net::AcceptResult, rbitcoin_net::AcceptError>;
}

impl Admit for rbitcoin_net::MempoolHub {
    fn admit(
        &self,
        tx: &bitcoin::Transaction,
    ) -> Result<rbitcoin_net::AcceptResult, rbitcoin_net::AcceptError> {
        self.accept_with_utxo(
            tx,
            &self.utxo_provider(),
            rbitcoin_net::TxOrigin::Relay(None),
            rbitcoin_net::tx_relay::AcceptPath::Relay,
            &[],
        )
    }
}
