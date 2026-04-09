// Bug: Null pointer panic in dealloc_pages
// Severity: medium
// Run: cargo xtask arceos qemu --package arceos-axalloc-null-pointer-panic-in-dealloc-pages --arch riscv64
// Expected (bug): kernel panics — dealloc_pages(0, ...) triggers NonNull::new(0).unwrap()
// Expected (fixed): returns gracefully without panicking

#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

use axalloc::{global_allocator, UsageKind};

fn poc_null_dealloc() {
    println!("Calling dealloc_pages(pos=0, num_pages=1)...");
    // Bug trigger: dealloc_pages internally does NonNull::new(0).unwrap()
    global_allocator().dealloc_pages(0, 1, UsageKind::RustHeap);
    println!("PASS: no panic — null pointer handled gracefully");
}

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("PoC: Null pointer panic in dealloc_pages");
    poc_null_dealloc();
}
