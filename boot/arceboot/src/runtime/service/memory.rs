use alloc::vec::Vec;

use ax_hal::{
    mem::{PhysAddr, VirtAddr},
    paging::MappingFlags,
};
use ax_memory_addr::VirtAddrRange;
use ax_sync::Mutex;
use uefi_raw::table::boot::{AllocateType, MemoryType};

static ALLOCATED_PAGES: Mutex<Vec<(VirtAddr, usize)>> = Mutex::new(Vec::new());
static ALLOCATED_POOLS: Mutex<Vec<(usize, core::alloc::Layout)>> = Mutex::new(Vec::new());

pub fn alloc_pages(_alloc_type: AllocateType, _memory_type: MemoryType, count: usize) -> *mut u8 {
    let size = count * 4096;
    let mut aspace = ax_mm::kernel_aspace().lock();
    // Map a fresh RWX region above the RAM linear-mapping window instead of
    // `protect`ing heap pages: protecting a range that shares a 2 MiB linear
    // mapping with the page tables themselves faults while the split is in
    // flight (the physical pages backing the PTE writes become unmapped).
    let hint = VirtAddr::from_usize(0x9000_0000);
    let limit = VirtAddrRange::from_start_size(VirtAddr::from_usize(0), usize::MAX);
    let va = aspace
        .find_free_area(hint, size, limit)
        .expect("no free VA for EFI pages");
    aspace
        .map_alloc(
            va,
            size,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
            true,
        )
        .expect("failed to map EFI pages");
    ALLOCATED_PAGES.lock().push((va, size));
    va.as_mut_ptr()
}

pub fn free_pages(_addr: PhysAddr, _page: usize) {
    // The EFI page mappings are single-shot allocations in a bootloader; the
    // UEFI `FreePages` service is intentionally a no-op here.
}

pub fn allocate_pool(_memory_type: MemoryType, size: usize) -> *mut u8 {
    if size == 0 {
        return core::ptr::null_mut();
    }
    // UEFI requires at least 8-byte alignment for pool allocations.
    let layout = match core::alloc::Layout::from_size_align(size, 8) {
        Ok(l) => l,
        Err(_) => return core::ptr::null_mut(),
    };
    let ptr = match ax_alloc::global_allocator().alloc(layout) {
        Ok(nn) => nn.as_ptr(),
        Err(_) => return core::ptr::null_mut(),
    };
    ALLOCATED_POOLS.lock().push((ptr as usize, layout));
    ptr
}

pub fn free_pool(buffer: *mut u8) {
    if buffer.is_null() {
        return;
    }
    let addr = buffer as usize;
    let mut pools = ALLOCATED_POOLS.lock();
    if let Some(idx) = pools.iter().position(|(p, _)| *p == addr) {
        let (_, layout) = pools.swap_remove(idx);
        // Safety: pointer/layout came from our allocator.
        unsafe {
            ax_alloc::global_allocator().dealloc(core::ptr::NonNull::new_unchecked(buffer), layout)
        };
    }
}
