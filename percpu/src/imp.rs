use core::sync::atomic::{AtomicBool, Ordering};

#[path = "tp.rs"]
mod tp;
pub use tp::*;

static IS_INIT: AtomicBool = AtomicBool::new(false);

const fn align_up_64(val: usize) -> usize {
    const SIZE_64BIT: usize = 0x40;
    (val + SIZE_64BIT - 1) & !(SIZE_64BIT - 1)
}

#[cfg(not(target_os = "none"))]
static PERCPU_AREA_BASE: spin::once::Once<usize> = spin::once::Once::new();

extern "C" {
    fn _percpu_start();
    fn _percpu_end();
    fn _percpu_load_start();
    fn _percpu_load_end();
}

/// Returns the number of per-CPU data areas reserved.
pub fn percpu_area_num() -> usize {
    (_percpu_end as usize - _percpu_start as usize) / align_up_64(percpu_area_size())
}

/// Returns the per-CPU data area size for one CPU.
pub fn percpu_area_size() -> usize {
    // It seems that `_percpu_load_start as usize - _percpu_load_end as usize` will result in more instructions.
    use percpu_macros::percpu_symbol_offset;
    percpu_symbol_offset!(_percpu_load_end) - percpu_symbol_offset!(_percpu_load_start)
}

/// Returns the base address of the per-CPU data area on the given CPU.
///
/// if `cpu_id` is 0, it returns the base address of all per-CPU data areas.
pub fn percpu_area_base(cpu_id: usize) -> usize {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "none")] {
            let base = _percpu_start as usize;
        } else {
            let base = *PERCPU_AREA_BASE.get().unwrap();
        }
    }
    base + cpu_id * align_up_64(percpu_area_size())
}

/// Initialize all per-CPU data areas.
///
/// The number of areas is determined by the following formula:
///
/// ```text
/// (percpu_section_size / align_up(percpu_area_size, 64)
/// ```
///
/// Returns the number of areas initialized. If this function has been called
/// before, it does nothing and returns 0.
pub fn init(_cpu_count: usize) -> usize {
    // avoid re-initialization.
    if IS_INIT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return 0;
    }

    #[cfg(target_os = "linux")]
    {
        // we not load the percpu section in ELF, allocate them here.
        let total_size = _percpu_end as usize - _percpu_start as usize;
        let layout = std::alloc::Layout::from_size_align(total_size, 0x1000).unwrap();
        PERCPU_AREA_BASE.call_once(|| unsafe { std::alloc::alloc(layout) as usize });
    }

    let base = percpu_area_base(0);
    let size = percpu_area_size();
    let num = percpu_area_num();
    for i in 1..num {
        let secondary_base = percpu_area_base(i);
        #[cfg(target_os = "none")]
        assert!(secondary_base + size <= _percpu_end as usize);
        // copy per-cpu data of the primary CPU to other CPUs.
        unsafe {
            core::ptr::copy_nonoverlapping(base as *const u8, secondary_base as *mut u8, size);
        }
    }
    num
}

/// Initializes the per-CPU data register.
///
/// It is equivalent to `write_percpu_reg(percpu_area_base(cpu_id))`, which set
/// the architecture-specific per-CPU data register to the base address of the
/// corresponding per-CPU data area.
///
/// `cpu_id` indicates which per-CPU data area to use.
pub fn init_percpu_reg(cpu_id: usize) {
    let tp = percpu_area_base(cpu_id);
    unsafe { write_percpu_reg(tp) }
}
