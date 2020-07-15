use std::sync::{Arc, atomic::{AtomicUsize, Ordering as AtomicOrdering}};
use std::{rc::Rc, cell::RefCell, collections::HashMap};
use wasmtime::{Memory, Extern, Func, FuncType, ValType, Engine, Store, Module, Val, Instance};
use anyhow::{Result, anyhow};
use parking_lot::Mutex;

use crate::runner;

trait Callable {
    fn call(&self, params: &[Val], results: &mut [Val]) -> Result<(), wasmtime::Trap>;
}

#[derive(Clone)]
pub struct ModuleWrapper {
    module: Arc<Module>,
    counter: Arc<AtomicUsize>,
}

#[derive(Clone)]
pub struct RunningContext {
    forks: Arc<Mutex<HashMap<u32, tokio::task::JoinHandle<u64>>>>,
    store: Store,
    engine: Engine,
}

impl RunningContext {
    pub fn with_engine(engine: &Engine) -> Self {
        Self {
            engine: engine.clone(),
            store: Store::new(engine),
            forks: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

impl ModuleWrapper {
    pub fn new(module: Module) -> Self {
        ModuleWrapper {
            module: Arc::new(module),
            counter: Arc::new(0.into()),
        }
    }

    fn next_pid(&self) -> usize {
        self.counter.fetch_add(1, AtomicOrdering::SeqCst)
    }

    pub fn as_ref(&self) -> &Module {
        self.module.as_ref()
    }
}

struct Fork {
    module: ModuleWrapper,
    memory: RefCell<Option<Memory>>,
    entry_func: RefCell<Option<Func>>,
    forks: Arc<Mutex<HashMap<u32, tokio::task::JoinHandle<u64>>>>,
    engine: Engine,
}

impl Fork {
    pub fn new(running_context: &RunningContext, module: ModuleWrapper) -> Self {
        Self {
            module,
            memory: None.into(),
            entry_func: None.into(),
            forks: running_context.forks.clone(),
            engine: running_context.engine.clone(),
        }
    }
}

impl Callable for Fork {
    fn call(
        &self,
        params: &[Val],
        results: &mut [Val],
    ) -> Result<(), wasmtime::Trap> {
        let entry_point = params[0].unwrap_i32();
        let data_desc = params[1].unwrap_i64() as u64;

        let memb = self.memory.borrow();
        let memory = memb.as_ref().expect("Memory should be set");

        let data_start = (data_desc >> 32) as usize;
        let data_len = (data_desc & 0x00000000FFFFFFFF) as usize;
        let data = unsafe { memory.data_unchecked()[data_start..data_start + data_len].to_vec() };

        let number = self.module.next_pid() as i32;
        let module_clone = self.module.clone();
        let engine = self.engine.clone();
        let handle = tokio::spawn(async move {
            runner::fork_module(
                &engine,
                module_clone,
                entry_point,
                data,
            ).expect("failed to execute fork") as u64
        });

        self.forks.lock().insert(number as u32, handle);

        results[0] = number.into();

        Ok(())
    }
}

pub trait PostInitialize {
    fn post_initialize(&self, instance: &Instance);
}

impl PostInitialize for Fork {
    fn post_initialize(&self, instance: &Instance) {
        *(self.memory.borrow_mut()) = Some(get_linear_memory(instance).expect("Memory should be exproted"));

        let entry_func = instance
            .get_export("invoke")
            .and_then(|e| e.into_func())
            .expect("fork dispatch 'invoke' should exist");
        *(self.entry_func.borrow_mut()) = Some(entry_func.clone());
    }
}

impl PostInitialize for Debug {
    fn post_initialize(&self, instance: &Instance) {
        *(self.memory.borrow_mut()) = Some(get_linear_memory(instance).expect("Memory should be exproted"));
    }
}

struct Debug {
    id: usize,
    memory: RefCell<Option<Memory>>,
}

impl Callable for Debug {
    fn call(
        &self,
        wasmtime_params: &[Val],
        _: &mut [Val],
    ) -> Result<(), wasmtime::Trap> {

        let memb = self.memory.borrow();
        let memory = memb.as_ref().expect("Memory should be set");

        let ptr: i32 = wasmtime_params[0].unwrap_i32();
        let len: i32 = wasmtime_params[1].unwrap_i32();

        let slc = unsafe { &memory.data_unchecked()[ptr as usize..(ptr+len) as usize] };
        println!("[DEBUG (pid#{})]: {}", self.id, unsafe { std::str::from_utf8_unchecked(slc) });

        Ok(())
    }
}

pub fn generate_imports(
    running_context: &RunningContext,
    module: ModuleWrapper,
) -> (Vec<Rc<dyn PostInitialize>>, Vec<Extern>) {
    let fork = Rc::new(Fork::new(running_context, module.clone()));
    let fork_clone = fork.clone();
    let fork_extern = Extern::Func(Func::new(
        &running_context.store,
        FuncType::new(Box::new([ValType::I32, ValType::I64]), Box::new([ValType::I32])),
        move |_caller, params, results| fork_clone.call(params, results),
    ));

    let debug = Rc::new(Debug { id: module.next_pid(), memory: None.into() });
    let debug_clone = debug.clone();
    let debug_extern = Extern::Func(Func::new(
        &running_context.store,
        FuncType::new(Box::new([ValType::I32, ValType::I32]), Box::new([])),
        move |_caller, params, results| debug_clone.call(params, results),
    ));

    (
        vec![debug, fork],
        vec![debug_extern, fork_extern],
    )
}

pub fn post_initialize(
    generated: &[Rc<dyn PostInitialize>],
    instance: &Instance,
) {
    for ext in generated {
        ext.post_initialize(instance);
    }
}

pub fn get_linear_memory(instance: &Instance) -> Result<Memory> {
	let memory_export = instance
		.get_export("memory")
		.ok_or(anyhow!("memory is not exported under `memory` name"))?;

	let memory = memory_export
		.into_memory()
		.ok_or(anyhow!("the `memory` export should have memory type"))?
		.clone();

	Ok(memory)
}
