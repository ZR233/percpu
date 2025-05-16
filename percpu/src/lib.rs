#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

extern crate percpu_macros;

cfg_if::cfg_if! {
    if #[cfg(feature = "sp-naive")] {
        #[path = "naive.rs"]
        mod imp;
    } else if #[cfg(feature = "custom-tp")] {
        #[path = "imp_custom.rs"]
        mod imp;
    }else{
        mod imp;
    }
}

use core::ptr::NonNull;

pub use self::imp::*;
pub use percpu_macros::def_percpu;

#[doc(hidden)]
pub mod __priv {
    #[cfg(feature = "preempt")]
    pub use kernel_guard::NoPreempt as NoPreemptGuard;
}

cfg_if::cfg_if! {
    if #[cfg(doc)] {
        /// Example per-CPU data for documentation only.
        #[cfg_attr(docsrs, doc(cfg(doc)))]
        #[def_percpu]
        pub static EXAMPLE_PERCPU_DATA: usize = 0;
    }
}

pub trait Impl {
    fn percpu_base() -> NonNull<u8>;
    fn set_cpu_local_ptr(ptr: *mut u8);
    fn get_cpu_local_ptr() -> *mut u8;
}
