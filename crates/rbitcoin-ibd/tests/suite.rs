rbitcoin_store::skip_flushes_before_main!();

mod frontier_competing;
mod frontier_replay;
mod ibd;
mod ibd_smoke;
mod peer_header_sync_journey;

/// Mine through [`rbitcoin_net::chain::ChainHub::generate_to_script_for_rpc_async`],
/// the path RPC `generate*` ships. The task runs on a suite runtime and the
/// caller blocks until it returns, so a plain test thread can call it.
pub(crate) trait Mine {
    fn mine(
        &self,
        nblocks: u32,
        script_pubkey: bitcoin::ScriptBuf,
        txs: Vec<bitcoin::Transaction>,
    ) -> Result<Vec<bitcoin::BlockHash>, rbitcoin_net::error::NetError>;
}

impl Mine for std::sync::Arc<rbitcoin_net::chain::ChainHub> {
    fn mine(
        &self,
        nblocks: u32,
        script_pubkey: bitcoin::ScriptBuf,
        txs: Vec<bitcoin::Transaction>,
    ) -> Result<Vec<bitcoin::BlockHash>, rbitcoin_net::error::NetError> {
        static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
        let rt = RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("mine runtime")
        });
        let hub = std::sync::Arc::clone(self);
        let (tx, rx) = std::sync::mpsc::channel();
        rt.spawn(async move {
            let mined = hub
                .generate_to_script_for_rpc_async(nblocks, script_pubkey, txs, false)
                .await;
            // Release the clone before the caller wakes, so it can unwrap.
            drop(hub);
            let _ = tx.send(mined);
        });
        rx.recv().expect("mine task")
    }
}
