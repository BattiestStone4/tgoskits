// Bug: Usage statistics updated before actual deallocation
// Severity: medium
// Run: cargo xtask arceos qemu --package arceos-axalloc-usage-statistics-updated-before-actual --arch riscv64
// Expected: Bug is code-level — stats decremented before inner.dealloc()
// If inner.dealloc() panics, stats become inconsistent

#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

use axalloc::{global_allocator, UsageKind};
use core::alloc::Layout;

fn poc_stats_ordering() {
    let alloc = global_allocator();

    // Test 1: byte alloc/dealloc
    let layout = Layout::from_size_align(1024, 8).unwrap();
    match alloc.alloc(layout) {
        Ok(ptr) => {
            println!("Allocated 1024 bytes at {:?}", ptr);
            // Bug: dealloc updates stats BEFORE inner.dealloc()
            unsafe { alloc.dealloc(ptr, layout) };
            println!("Deallocated");
        }
        Err(e) => println!("Alloc failed: {:?}", e),
    }

    // Test 2: page alloc/dealloc
    match alloc.alloc_pages(4, 4096, UsageKind::RustHeap) {
        Ok(addr) => {
            println!("Allocated 4 pages at {:x}", addr);
            alloc.dealloc_pages(addr, 4, UsageKind::RustHeap);
            println!("Deallocated");
        }
        Err(e) => println!("Page alloc failed: {:?}", e),
    }

    println!("");
    println!("Bug: dealloc() updates usage stats BEFORE inner.dealloc()");
    println!("If inner.dealloc() panics, stats show freed but memory still allocated");
}

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("PoC: Stats updated before actual dealloc");
    poc_stats_ordering();
}
