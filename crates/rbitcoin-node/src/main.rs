use rbitcoin_node::process_main;
use std::process::ExitCode;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[doc(hidden)]
pub fn main() -> ExitCode {
    process_main()
}
