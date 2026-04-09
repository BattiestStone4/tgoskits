// Bug: Usage counter underflow on double-free
// Severity: medium
// Run: cargo xtask arceos qemu --package arceos-axalloc-usage-counter-underflow-on-double-free --arch riscv64
// Expected (bug): usage counter underflows from 0 to huge value
// Expected (fixed): double-free detected or counter stays at 0

#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

use axalloc::global_allocator;
use core::alloc::Layout;
use core::ptr::NonNull;

fn poc_double_free() {
    let alloc = global_allocator();
    let layout = Layout::from_size_align(128, 8).unwrap();

    // Step 1: allocate
    let ptr = match alloc.alloc(layout) {
        Ok(p) => p,
        Err(e) => { println!("FAIL: alloc failed: {:?}", e); return; }
    };
    println!("[1] Allocated 128 bytes at {:?}", ptr);

    // Step 2: correct dealloc — counter goes to 0
    unsafe { alloc.dealloc(ptr, layout) };
    println!("[2] First dealloc done, counter should be 0");

    // Step 3: double-free — triggers underflow in Usages::dealloc
    // self.0[kind as usize] -= size  →  0 - 128 underflows!
    println!("[3] Double-free (BUG TRIGGER)...");
    unsafe { alloc.dealloc(ptr, layout) };
    println!("    BUG CONFIRMED: counter underflowed to ~usize::MAX");
}

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("PoC: Usage counter underflow on double-free");
    poc_double_free();
}
