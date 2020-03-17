mod host;
mod runner;

use anyhow::Result;
use wasmtime::{Module, Store};

#[tokio::main]
async fn main() -> Result<()> {
    let file_name = std::env::args().nth(1).expect("wfork <filename.wasm>");

    runner::run_module(host::ModuleWrapper::from(Module::from_file(&Store::default(), file_name)?))?;

    Ok(())
}