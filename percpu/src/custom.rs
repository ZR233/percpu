use core::{
    cell::UnsafeCell,
    fmt::{Debug, Display},
    sync::atomic::{AtomicBool, Ordering},
};

#[path = "tp.rs"]
mod tp;

pub use tp::*;

#[cfg(feature = "preempt")]
use kernel_guard::NoPreempt;

static IS_INIT: AtomicBool = AtomicBool::new(false);
static mut PERCPU_SIZE: usize = 0;
static mut PERCPU_BASE: usize = 0;

#[repr(transparent)]
pub struct PerCpuData<T> {
    data: UnsafeCell<T>,
}

unsafe impl<T> Sync for PerCpuData<T> {}
unsafe impl<T> Send for PerCpuData<T> {}

impl<T> PerCpuData<T> {
    /// Creates a new per-CPU static variable with the given initial value.
    pub const fn new(data: T) -> PerCpuData<T> {
        PerCpuData {
            data: UnsafeCell::new(data),
        }
    }

    /// Returns the offset relative to the per-CPU data area base.
    #[inline]
    pub fn offset(&self) -> usize {
        self.data.get() as usize - percpu_link_start()
    }

    /// Returns the raw pointer of this per-CPU static variable on the given CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that
    /// - the CPU ID is valid, and
    /// - data races will not happen.
    #[inline]
    pub fn remote_ptr(&self, cpu_idx: usize) -> *mut T {
        let addr = percpu_base() + cpu_idx * percpu_area_size() + self.offset();
        addr as *mut T
    }

    /// Returns the raw pointer of this per-CPU static variable on the current CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that preemption is disabled on the current CPU.
    #[inline]
    pub unsafe fn current_ptr(&self) -> *mut T {
        let addr = read_percpu_reg() + self.offset();
        addr as *mut T
    }

    /// Returns the reference of the per-CPU static variable on the current CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that preemption is disabled on the current CPU.
    #[inline]
    pub unsafe fn current_ref_raw(&self) -> &T {
        &*self.current_ptr()
    }

    /// Returns the mutable reference of the per-CPU static variable on the current CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that preemption is disabled on the current CPU.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn current_ref_mut_raw(&self) -> &mut T {
        unsafe { &mut *self.current_ptr() }
    }

    /// Set the value of the per-CPU static variable on the current CPU. Preemption will be disabled during the
    /// call.
    pub fn write_current(&self, val: T) {
        #[cfg(feature = "preempt")]
        let _g = NoPreempt::new();
        unsafe { self.write_current_raw(val) };
    }

    /// Set the value of the per-CPU static variable on the current CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that preemption is disabled on the current CPU.
    pub unsafe fn write_current_raw(&self, val: T) {
        unsafe {
            *self.current_ptr() = val;
        }
    }

    /// Write the value to the per-CPU variable on the specified CPU.
    ///
    /// # Safety
    ///
    /// This function should called with a mutex or before the cpu is online.
    pub unsafe fn write_remote(&self, cpu_idx: usize, val: T) {
        #[cfg(feature = "preempt")]
        let _g = NoPreempt::new();
        unsafe {
            *self.remote_ptr(cpu_idx) = val;
        }
    }

    /// Manipulate the per-CPU data on the current CPU in the given closure.
    /// Preemption will be disabled during the call.
    pub fn with_current<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        #[cfg(feature = "preempt")]
        let _g = NoPreempt::new();
        unsafe { f(&mut *self.current_ptr()) }
    }

    /// Returns the reference of the per-CPU static variable on the given CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that
    /// - the CPU ID is valid, and
    /// - data races will not happen.
    #[inline]
    pub unsafe fn remote_ref_raw(&self, cpu_id: usize) -> &T {
        &*self.remote_ptr(cpu_id)
    }

    /// Returns the mutable reference of the per-CPU static variable on the given CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that
    /// - the CPU ID is valid, and
    /// - data races will not happen.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn remote_ref_mut_raw(&self, cpu_id: usize) -> &mut T {
        &mut *self.remote_ptr(cpu_id)
    }
}

impl<T: Clone> PerCpuData<T> {
    /// Returns the value of the per-CPU static variable on the current CPU. Preemption will be disabled during
    /// the call.
    pub fn read_current(&self) -> T {
        #[cfg(feature = "preempt")]
        let _g = NoPreempt::new();
        unsafe { self.read_current_raw() }
    }

    /// Returns the value of the per-CPU static variable on the current CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that preemption is disabled on the current CPU.
    pub unsafe fn read_current_raw(&self) -> T {
        unsafe { (*self.current_ptr()).clone() }
    }

    /// Returns the value of the per-CPU static variable on the given CPU.
    pub fn read_remote(&self, cpu_idx: usize) -> T {
        #[cfg(feature = "preempt")]
        let _g = NoPreempt::new();
        unsafe { (*self.remote_ptr(cpu_idx)).clone() }
    }
}

impl<T: Debug> Debug for PerCpuData<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[cfg(feature = "preempt")]
        let _g = NoPreempt::new();

        unsafe { &*self.data.get() }.fmt(f)
    }
}

impl<T: Display> Display for PerCpuData<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[cfg(feature = "preempt")]
        let _g = NoPreempt::new();

        unsafe { &*self.data.get() }.fmt(f)
    }
}

unsafe extern "C" {
    fn _percpu_load_start();
    fn _percpu_load_end();
}

/// Returns the size of the per-CPU link section.
pub fn percpu_section_size() -> usize {
    _percpu_load_end as usize - _percpu_load_start as usize
}

#[inline]
fn percpu_base() -> usize {
    unsafe { PERCPU_BASE }
}

#[inline]
fn percpu_link_start() -> usize {
    _percpu_load_start as usize
}

/// Returns the per-CPU data area size for one CPU.
#[inline]
pub fn percpu_area_size() -> usize {
    unsafe { PERCPU_SIZE }
}

fn get_actual_percpu_base() -> usize {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "linux")]{
            _linux::percpu_base()
        }else{
            unsafe extern "C" {
                fn _percpu_base() -> *mut u8;
            }
            unsafe{
                _percpu_base() as _
            }
        }
    }
}

/// Initialize all per-CPU data areas.
///
/// Returns the number of areas initialized. If this function has been called
/// before, it does nothing and returns 0.
pub fn init(cpu_count: usize) -> usize {
    #[cfg(target_os = "linux")]
    _linux::init(cpu_count);

    unsafe {
        // avoid re-initialization.
        if IS_INIT
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return 0;
        }

        PERCPU_BASE = get_actual_percpu_base();
        PERCPU_SIZE = percpu_section_size();

        let src = core::slice::from_raw_parts(percpu_link_start() as *const u8, percpu_area_size());

        for i in 0..cpu_count {
            let ptr = (percpu_base() + i * PERCPU_SIZE) as *mut u8;

            let dst = core::slice::from_raw_parts_mut(ptr, percpu_area_size());

            if i == 0 && dst.eq(&src) {
                continue;
            }
            dst.copy_from_slice(src);
        }
    }
    cpu_count
}

/// Initializes the per-CPU data register.
///
/// It is equivalent to `write_percpu_reg(percpu_area_base(cpu_id))`, which set
/// the architecture-specific per-CPU data register to the base address of the
/// corresponding per-CPU data area.
///
/// `cpu_id` indicates which per-CPU data area to use.
pub fn init_percpu_reg(cpu_idx: usize) {
    unsafe {
        let ptr = percpu_area_base(cpu_idx);
        write_percpu_reg(ptr);
    }
}

/// Returns the base address of the per-CPU data area on the given CPU.
///
/// Always returns `0` for "sp-naive" use.
pub fn percpu_area_base(cpu_idx: usize) -> usize {
    percpu_base() + cpu_idx * percpu_area_size()
}

#[cfg(target_os = "linux")]
mod _linux {
    use std::sync::Mutex;

    use super::*;

    static PERCPU_DATA: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    static mut PERCPU_BASE: usize = 0;

    pub fn percpu_base() -> usize {
        unsafe { PERCPU_BASE }
    }

    pub fn init(cpu_count: usize) {
        let size = cpu_count * percpu_section_size();
        let mut g = PERCPU_DATA.lock().unwrap();
        g.resize(size, 0);

        unsafe {
            let base = g.as_slice().as_ptr() as usize;
            PERCPU_BASE = base;
            println!("alloc percpu data @{:#x}, size: {:#x}", base, size);
        }
    }
}
