# wasmtime-fork

Proof-of-concept wasmtime host that can fork execution of WASM v1.

This is based on Parity fork of wasmtime.

## How it works.

1. Host provides an import
```
fork(i32, i64) -> i32
```
    Those are 3 arguments encoded (2 of them compactified to last argument):
    - entry point of the new spawned process (function item).
    - payload pointer passed to the entry point.
    - payload length passed to the entry point.

2. Embedded program calls to the `fork(&some_func, &payload)`.

3. Host spawns new thread and creates new instance in that thread based on the same compiled module (this why we use fork - so that wasmtime compiled `Module` could be `Send`).

4. Host allocates memory inside child instance and writes payload to it (embedded progam should expose `allocate` function)

5. Host starts `invoke` function inside child instance and passes entry_point that parent process originally used.

6. Parent and child now run in parallel!
