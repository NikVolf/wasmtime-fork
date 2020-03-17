
mod ext {
    #[link(wasm_import_module = "env")]
    extern "C" {
        pub fn debug(ptr: *const u8, len: u32);
        pub fn fork(ptr: usize, payload: u64) -> u32;
    }
}

fn debug(s: &str) {
    unsafe { ext::debug(s.as_ptr(), s.len() as u32) }
}

fn fork(entry_point: fn(u64) -> u64, data: Vec<u8>) -> u32 {
    #[no_mangle]
    unsafe extern "C" fn invoke(entry: usize, payload: u64) -> u64 {
        let ptr: fn(u64) -> u64 = std::mem::transmute(entry);
        (ptr)(payload)
    }

    unsafe {
        let ptr: usize = std::mem::transmute(entry_point);
        let vec_ptr = data.as_ptr() as usize as u64;
        let data_encode = (vec_ptr << 32) + (data.len() as u64);
        ext::fork(ptr, data_encode)
    }
}

fn fork_entry_point1(data: u64) -> u64 {
    let data_ptr = (data >> 32) as usize as *const u8;
    let data_len = (data & 0x00000000FFFFFFFF) as usize;
    let input = unsafe { std::slice::from_raw_parts(data_ptr, data_len) };
    let mut v = 0;
    for i in 1..1000 {
        debug(&format!("forked with message: {}/{}", unsafe { std::str::from_utf8_unchecked(input) }, v));
        for t in 0..100000000 { v = (v + t) % i}
    }
    0
}

#[no_mangle]
unsafe extern "C" fn run() {
    debug("started");
    let _handle3 = fork(fork_entry_point1, "fork rules!".as_bytes().to_vec());
    debug("done");
}

fn main() {
}
