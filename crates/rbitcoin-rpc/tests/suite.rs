rbitcoin_store::skip_flushes_before_main!();

mod auth;
mod blockstats;
mod methods;
mod root;
mod server;

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
