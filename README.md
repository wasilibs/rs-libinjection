# rs-libinjection

rs-libinjection is a library for libinjection which wraps the C [libinjection][2] library.
It is packaged as a WebAssembly module and accessed with the runtime,  [wasmtime][3]. This means that it is 
compatible with any Rust application, without needing to provide the C libinjection library itself or
setting LDFLAGS.

This repository is currently under heavy construction.

[2]: https://github.com/libinjection/libinjection
[3]: https://wasmtime.dev/