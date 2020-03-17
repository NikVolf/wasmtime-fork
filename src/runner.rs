use anyhow::Result;
use crate::host;
use wasmtime::{Store, Instance};

pub fn run_module(module: host::ModuleWrapper) -> Result<()> {
    let store = Store::default();
    let (imports, externs) = host::generate_imports(&store, module.clone());
    let instance = Instance::new(module.as_ref(), &externs)?;
    host::post_initialize(&imports, &instance);
    run_instance(&instance)?;

    Ok(())
}

pub fn fork_module(module: host::ModuleWrapper, entry_point: i32, arguments: i64) -> Result<i64> {
    let store = Store::default();
    let (imports, externs) = host::generate_imports(&store, module.clone());
    let instance = Instance::new(module.as_ref(), &externs)?;
    host::post_initialize(&imports, &instance);

    let invoke = instance
        .get_export("invoke")
        .and_then(|e| e.func())
        .ok_or(anyhow::format_err!("failed to find `run` function export"))?
        .get2::<i32, i64, i64>()?;

    let result = invoke(entry_point, arguments)?;

    Ok(result)
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