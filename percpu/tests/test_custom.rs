#![cfg(all(feature = "custom-tp", any(target_os = "linux", target_os = "windows")))]

use percpu::*;

// Initial value is unsupported for testing.

#[def_percpu]
static U8: u8 = 1;

#[def_percpu]
static U16: u16 = 2;

#[derive(Clone)]
struct Struct {
    foo: usize,
    bar: u8,
}

#[def_percpu]
static STRUCT: Struct = Struct { foo: 10, bar: 11 };

#[cfg(target_os = "linux")]
pub mod test_linux {
    extern crate std;

    use std::{
        collections::HashMap,
        sync::{LazyLock, Mutex},
        thread::ThreadId,
    };

    use super::*;

    pub const CPU_COUNT: usize = 4;

    static CPU_DATA: Mutex<Vec<u8>> = Mutex::new(vec![]);
    static BASAE: Mutex<usize> = Mutex::new(0);
    static CPU_LOCAL_REG: LazyLock<Mutex<HashMap<ThreadId, usize>>> = LazyLock::new(|| {
        let base = {
            let mut g = CPU_DATA.lock().unwrap();
            g.resize(CPU_COUNT * percpu_section_size(), 0);
            g.as_ptr()
        };

        println!("percpu base: {:p}", base);
        {
            let mut g = BASAE.lock().unwrap();
            *g = base as usize;
        }
        init_data(CPU_COUNT);

        Mutex::new(HashMap::new())
    });

    pub struct PerCpuImpl;

    impl Impl for PerCpuImpl {
        fn percpu_base() -> std::ptr::NonNull<u8> {
            std::ptr::NonNull::new(*BASAE.lock().unwrap() as *mut u8).unwrap()
        }

        fn set_cpu_local_ptr(ptr: *mut u8) {
            let id = std::thread::current().id();
            CPU_LOCAL_REG.lock().unwrap().insert(id, ptr as _);
        }

        fn get_cpu_local_ptr() -> *mut u8 {
            let id = std::thread::current().id();
            let ptr = CPU_LOCAL_REG
                .lock()
                .unwrap()
                .get(&id)
                .cloned()
                .unwrap_or_default();
            ptr as _
        }
    }

    pub fn init() {
        drop(CPU_LOCAL_REG.lock());
    }
}

#[cfg(target_os = "linux")]
impl_percpu!(test_linux::PerCpuImpl);

#[cfg(target_os = "linux")]
#[test]
fn test_percpu() {
    extern crate std;
    test_linux::init();

    for i in 0..test_linux::CPU_COUNT {
        let handle = std::thread::spawn(move || {
            init(i);

            assert_eq!(U8.read_current(), 1);
            assert_eq!(U16.read_current(), 2);
            assert_eq!(STRUCT.read_current().foo, 10);
            assert_eq!(STRUCT.read_current().bar, 11);

            U8.write_current(3);

            assert_eq!(U8.read_current(), 3);
        });

        handle.join().unwrap();
    }
}
