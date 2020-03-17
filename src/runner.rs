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

pub fn fork_module(module: host::ModuleWrapper, entry_point: i32, payload: Vec<u8>) -> Result<i64> {
    let store = Store::default();
    let (imports, externs) = host::generate_imports(&store, module.clone());
    let instance = Instance::new(module.as_ref(), &externs)?;
    host::post_initialize(&imports, &instance);

    let invoke = instance
        .get_export("invoke")
        .and_then(|e| e.func())
        .ok_or(anyhow::format_err!("failed to find `invoke` function export"))?
        .get2::<i32, i64, i64>()?;

    let allocate = instance
        .get_export("allocate")
        .and_then(|e| e.func())
        .ok_or(anyhow::format_err!("failed to find `allocate` function export"))?
        .get1::<i32, i32>()?;

    let payload_ptr = allocate(payload.len() as i32)?;
    let memory = host::get_linear_memory(&instance)?;
    unsafe {
        memory.data_unchecked_mut()[payload_ptr as usize..payload_ptr as usize +payload.len()]
            .copy_from_slice(&payload[..]);
    }

    let arguments = ((payload_ptr as u64) << 32) + payload.len() as u64;

    let result = invoke(entry_point, arguments as i64)?;

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