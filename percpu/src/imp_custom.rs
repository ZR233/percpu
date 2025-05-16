use core::{
    cell::UnsafeCell,
    fmt::{Debug, Display},
};

#[cfg(feature = "preempt")]
use kernel_guard::NoPreempt;

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
        let addr = percpu_base() + cpu_idx * percpu_size() + self.offset();
        addr as *mut T
    }

    /// Returns the raw pointer of this per-CPU static variable on the current CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that preemption is disabled on the current CPU.
    #[inline]
    pub unsafe fn current_ptr(&self) -> *mut T {
        let addr = get_cpu_local_ptr() as usize + self.offset();
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
    fn _percpu_base() -> *mut u8;
    fn _percpu_set_cpu_local_ptr(ptr: *mut u8);
    fn _percpu_get_cpu_local_ptr() -> *mut u8;
}

pub fn percpu_section_size() -> usize {
    _percpu_load_end as usize - _percpu_load_start as usize
}

#[inline]
fn percpu_base() -> usize {
    unsafe { _percpu_base() as usize }
}
#[inline]
fn percpu_link_start() -> usize {
    _percpu_load_start as usize
}

#[macro_export]
macro_rules! impl_percpu {
    ($impl:ty) => {
        #[unsafe(no_mangle)]
        #[inline]
        pub extern "C" fn _percpu_base() -> *mut u8 {
            <$impl as $crate::Impl>::percpu_base().as_ptr()
        }
        #[unsafe(no_mangle)]
        #[inline]
        pub extern "C" fn _percpu_set_cpu_local_ptr(ptr: *mut u8) {
            <$impl as $crate::Impl>::set_cpu_local_ptr(ptr)
        }
        #[unsafe(no_mangle)]
        #[inline]
        pub extern "C" fn _percpu_get_cpu_local_ptr() -> *mut u8 {
            <$impl as $crate::Impl>::get_cpu_local_ptr()
        }
    };
}

static mut PERCPU_SIZE: usize = 0;

#[inline]
fn percpu_size() -> usize {
    unsafe { PERCPU_SIZE }
}

#[inline]
fn get_cpu_local_ptr() -> *mut u8 {
    unsafe { _percpu_get_cpu_local_ptr() }
}

pub fn init(cpu_count: usize) {
    unsafe {
        PERCPU_SIZE = percpu_section_size();

        let src = core::slice::from_raw_parts(percpu_link_start() as *const u8, percpu_size());

        for i in 0..cpu_count {
            let ptr = (percpu_base() + i * PERCPU_SIZE) as *mut u8;

            let dst = core::slice::from_raw_parts_mut(ptr, percpu_size());

            if i == 0 && dst.eq(&src) {
                continue;
            }
            dst.copy_from_slice(src);
        }
    }
}
pub fn init_percpu_reg(cpu_idx: usize) {
    unsafe {
        let ptr = (percpu_base() + cpu_idx * percpu_size()) as *mut u8;
        _percpu_set_cpu_local_ptr(ptr);
    }
}
