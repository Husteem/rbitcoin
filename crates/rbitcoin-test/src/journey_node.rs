//! The operator CLI (`journey-node`) as a child process.
//!
//! Stderr is the operator log. The child is killed on drop, and every wait
//! for exit has a deadline, so a node that ignores `stop` fails the test
//! instead of hanging it.

use serde_json::json;
use std::io::BufRead;
use std::net::SocketAddr;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// A running `journey-node` child.
pub struct JourneyNode {
    child: Child,
    stderr: Arc<Mutex<String>>,
    reader: Option<JoinHandle<()>>,
}

impl JourneyNode {
    /// Spawn `cmd` (the `journey-node` bin with its args) with stderr piped
    /// into the log.
    pub fn spawn(mut cmd: Command) -> Self {
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("spawn rbitcoin-node: {e}"));
        let pipe = child.stderr.take().expect("stderr piped");
        let stderr = Arc::new(Mutex::new(String::new()));
        let buf = Arc::clone(&stderr);
        let reader = std::thread::spawn(move || {
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
        Self {
            child,
            stderr,
            reader: Some(reader),
        }
    }

    /// The operator log so far.
    pub fn log(&self) -> String {
        self.stderr.lock().unwrap().clone()
    }

    /// RPC `stop`, then [`Self::wait_exit`].
    pub async fn stop(self, rpc: SocketAddr, wall: Duration) -> String {
        let _ = crate::jsonrpc::try_jsonrpc(rpc, "stop", json!([])).await;
        self.wait_exit(wall).await
    }

    /// Wait up to `wall` for a successful exit and return the whole log.
    /// Past `wall` the child is killed and the test panics.
    pub async fn wait_exit(mut self, wall: Duration) -> String {
        let deadline = Instant::now() + wall;
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("try_wait node") {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                self.join_reader();
                panic!(
                    "rbitcoin-node did not exit within {wall:?}\n--- node log ---\n{}",
                    self.log()
                );
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        self.join_reader();
        let log = self.log();
        assert!(status.success(), "rbitcoin-node exit {status}\n{log}");
        log
    }

    fn join_reader(&mut self) {
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

impl Drop for JourneyNode {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.join_reader();
    }
}
