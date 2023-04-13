use lazy_static::lazy_static;
use std::cell::RefCell;
use std::thread::LocalKey;
use wasmtime::{Engine, Linker, Memory, Module, Store, TypedFunc};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

static WASM: &[u8] = include_bytes!("libinjection.wasm");

struct Abi {
    memory: Memory,
    malloc: TypedFunc<u32, u32>,
    free: TypedFunc<u32, ()>,
    libinjection_sqli: TypedFunc<(u32, u32, u32), u32>,
    libinjection_xss: TypedFunc<(u32, u32), u32>,
}

lazy_static! {
    static ref WASM_COMPILED: Module = Module::new(&Engine::default(), WASM).unwrap();
}

thread_local! {
    static STORE: RefCell<Option<Store<WasiCtx>>> = RefCell::new(None);
    static ABI: RefCell<Option<Abi>> = RefCell::new(None);
}

pub fn is_sqli(input: &str) -> (bool, String) {
    with_abi(|mut store, abi| {
        let buffer = abi.malloc.call(&mut store, input.len() as u32 + 9).unwrap();
        abi.memory.data_mut(&mut store)[buffer as usize..buffer as usize + input.len()]
            .copy_from_slice(input.as_bytes());
        let result = abi
            .libinjection_sqli
            .call(
                &mut store,
                (buffer, input.len() as u32, buffer + input.len() as u32),
            )
            .unwrap();
        let fp = abi.memory.data(&store)
            [buffer as usize + input.len()..buffer as usize + input.len() + 9]
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as char)
            .collect::<String>();
        abi.free.call(&mut store, buffer).unwrap();
        return (result != 0, fp);
    })
}

pub fn is_xss(input: &str) -> bool {
    with_abi(|mut store, abi| {
        let buffer = abi.malloc.call(&mut store, input.len() as u32).unwrap();
        abi.memory.data_mut(&mut store)[buffer as usize..buffer as usize + input.len()]
            .copy_from_slice(input.as_bytes());
        let result = abi
            .libinjection_xss
            .call(&mut store, (buffer, input.len() as u32))
            .unwrap();
        abi.free.call(&mut store, buffer).unwrap();
        return result != 0;
    })
}

fn with_abi<'a, F, T>(f: F) -> T
where
    F: FnOnce(&mut Store<WasiCtx>, &Abi) -> T,
{
    get_abi().with(|abi| {
        let abi_ref = abi.borrow();
        let abi = abi_ref.as_ref().unwrap();
        get_store().with(|store| {
            let mut store_ref = store.borrow_mut();
            let mut store = store_ref.as_mut().unwrap();
            return f(store, abi);
        })
    })
}

fn get_store() -> &'static LocalKey<RefCell<Option<Store<WasiCtx>>>> {
    STORE.with(|store| {
        if store.borrow().is_none() {
            *store.borrow_mut() = Some(Store::new(
                &WASM_COMPILED.engine(),
                WasiCtxBuilder::new().build(),
            ));
        }
    });
    return &STORE;
}

fn get_abi() -> &'static LocalKey<RefCell<Option<Abi>>> {
    get_store().with(|store| {
        let mut store_ref = store.borrow_mut();
        let mut store = store_ref.as_mut().unwrap();
        ABI.with(|abi| {
            if abi.borrow().is_none() {
                let mut linker = Linker::new(WASM_COMPILED.engine());
                wasmtime_wasi::add_to_linker(&mut linker, |s| s).unwrap();
                linker.module(&mut store, "", &WASM_COMPILED).unwrap();
                let instance = linker.instantiate(&mut store, &WASM_COMPILED).unwrap();
                let malloc = instance
                    .get_typed_func::<u32, u32>(&mut store, "malloc")
                    .unwrap();
                let free = instance
                    .get_typed_func::<u32, ()>(&mut store, "free")
                    .unwrap();
                let libinjection_sqli = instance
                    .get_typed_func::<(u32, u32, u32), u32>(&mut store, "libinjection_sqli")
                    .unwrap();
                let libinjection_xss = instance
                    .get_typed_func::<(u32, u32), u32>(&mut store, "libinjection_xss")
                    .unwrap();
                let memory = instance.get_memory(&mut store, "memory").unwrap();
                *abi.borrow_mut() = Some(Abi {
                    memory,
                    malloc,
                    free,
                    libinjection_sqli,
                    libinjection_xss,
                });
            }
        })
    });
    return &ABI;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use testutil::get_sqli_tests;
    use threadpool::ThreadPool;

    #[test]
    fn sqli_basic() {
        let (res, fp) = is_sqli("this is not isqli");
        assert!(!res);
        let (res, fp) = is_sqli("this\nis a ' or ''='\nsql injection");
        assert!(res);
    }

    #[test]
    fn sqli_threads() {
        let threadpool = ThreadPool::new(12);
        for _ in 0..12 {
            threadpool.execute(|| {
                for _ in 0..1000 {
                    let (res, fp) = is_sqli("this is not isqli");
                    assert!(!res);
                    let (res, fp) = is_sqli("this\nis a ' or ''='\nsql injection");
                    assert!(res);
                }
            });
        }
        threadpool.join();
    }

    #[test]
    fn sqli_alltests() {
        for test in get_sqli_tests() {
            let (res, fp) = is_sqli(&test.input);
            if &test.expected == "" {
                assert!(!res);
            } else {
                assert!(res);
                assert_eq!(fp, test.expected);
            }
        }
    }

    #[rstest]
    #[case("<script>alert(1);</script>")]
    #[case("><script>alert(1);</script>")]
    #[case("x ><script>alert(1);</script>")]
    #[case("' ><script>alert(1);</script>")]
    #[case("\"><script>alert(1);</script>")]
    #[case("red;</style><script>alert(1);</script>")]
    #[case("red;}</style><script>alert(1);</script>")]
    #[case("red;\"/><script>alert(1);</script>")]
    #[case("');}</style><script>alert(1);</script>")]
    #[case("onerror=alert(1)>")]
    #[case("x onerror=alert(1);>")]
    #[case("x' onerror=alert(1);>")]
    #[case("x\" onerror=alert(1);>")]
    #[case("<a href=\"javascript:alert(1)\">")]
    #[case("<a href='javascript:alert(1)'>")]
    #[case("<a href=javascript:alert(1)>")]
    #[case("<a href  =   javascript:alert(1); >")]
    #[case("<a href=\"  javascript:alert(1);\" >")]
    #[case("<a href=\"JAVASCRIPT:alert(1);\" >")]
    fn xss_examples(#[case] input: &str) {
        assert!(is_xss(input));
    }
}
