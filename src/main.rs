mod host;

use anyhow::Result;
use wasmtime::{Module, Store, Instance};

#[tokio::main]
async fn main() -> Result<()> {
    let file_name = std::env::args().nth(1).expect("wfork <filename.wasm>");

    println!("Initializing...");
    let store = Store::default();

    let module = host::ModuleWrapper::from(Module::from_file(&store, file_name)?);

    let (imports, externs) = host::generate_imports(&store, module.clone());

    let instance = Instance::new(module.as_ref(), &externs)?;
    host::post_initialize(&imports, &instance);

    run_instance(&instance)?;

    Ok(())
}

fn run_instance(instance: &Instance) -> Result<()> {
    let run = instance
        .get_export("run")
        .and_then(|e| e.func())
        .ok_or(anyhow::format_err!("failed to find `run` function export"))?
        .get0::<()>()?;

    run()?;

    Ok(())
}