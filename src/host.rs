use std::sync::{Arc, atomic::{AtomicUsize, Ordering as AtomicOrdering}};
use std::{rc::Rc, cell::RefCell};
use wasmtime::{Callable, Memory, Extern, Func, FuncType, ValType, Store, Module, Val, Instance};
use anyhow::{Result, anyhow};

#[derive(Clone)]
pub struct ModuleWrapper {
    module: Arc<Module>,
    counter: Arc<AtomicUsize>,
}

impl From<Module> for ModuleWrapper {
    fn from(module: Module) -> Self {
        ModuleWrapper {
            module: Arc::new(module),
            counter: Arc::new(0.into()),
        }
    }
}

impl ModuleWrapper {
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
}

impl Callable for Fork {
    fn call(
        &self,
        params: &[Val],
        results: &mut [Val],
    ) -> Result<(), wasmtime::Trap> {
        let entry_point = params[0].unwrap_i32();
        let data = params[0].unwrap_i64();

        self.entry_func
            .borrow().as_ref().expect("should be set at this point")
            .call(&[entry_point.into(), data.into()]).expect("Forked func failed");

        results[0] = 0.into();

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
            .and_then(|e| e.func())
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
    store: &Store,
    module: ModuleWrapper,
) -> (Vec<Rc<dyn PostInitialize>>, Vec<Extern>) {
    let fork = Rc::new(Fork { module: module.clone(), memory: None.into(), entry_func: None.into() });
    let fork_extern = Extern::Func(Func::new(
        store,
        FuncType::new(Box::new([ValType::I32, ValType::I64]), Box::new([ValType::I32])),
        fork.clone(),
    ));

    let debug = Rc::new(Debug { id: module.next_pid(), memory: None.into() });
    let debug_extern = Extern::Func(Func::new(
        store,
        FuncType::new(Box::new([ValType::I32, ValType::I32]), Box::new([])),
        debug.clone(),
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
		.memory()
		.ok_or(anyhow!("the `memory` export should have memory type"))?
		.clone();

	Ok(memory)
}