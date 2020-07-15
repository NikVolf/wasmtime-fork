mod host;
mod runner;

use anyhow::Result;
use wasmtime::{Module, Store, Config, Engine};

#[tokio::main]
async fn main() -> Result<()> {
    let file_name = std::env::args().nth(1).expect("wfork <filename.wasm>");

    let config = Config::default();
    let engine = Engine::new(&config);

    runner::run_module(&engine, host::ModuleWrapper::new(Module::from_file(&engine, file_name)?))?;

    Ok(())
}