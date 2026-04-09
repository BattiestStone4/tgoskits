// Bug: Panic on null pointer in dealloc
// Severity: medium
// Run: cargo xtask arceos qemu --package arceos-axalloc-panic-on-null-pointer-in-globalalloc --arch riscv64
// Expected (bug): kernel panic — dealloc with null-ish address
// Expected (fixed): no panic

#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

use axalloc::global_allocator;
use core::alloc::Layout;
use core::ptr::NonNull;

fn poc_dealloc_null() {
    let alloc = global_allocator();
    let layout = Layout::new::<u8>();

    // Construct NonNull from address 0 — triggers the bug in dealloc
    // which does NonNull::new(raw_ptr).expect("dealloc null ptr")
    let null_nn = unsafe { NonNull::new_unchecked(0 as *mut u8) };
    println!("Calling dealloc with null pointer...");
    unsafe { alloc.dealloc(null_nn, layout) };
    println!("PASS: no panic on null dealloc");
}

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("PoC: Panic on null pointer in dealloc");
    poc_dealloc_null();
}
