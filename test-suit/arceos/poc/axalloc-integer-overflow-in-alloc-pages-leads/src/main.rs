// Bug: Integer overflow in alloc_pages leads to undersized allocation
// Severity: high
// Run: cargo xtask arceos qemu --package arceos-axalloc-integer-overflow-in-alloc-pages-leads --arch riscv64
// Expected (bug): alloc_pages succeeds with undersized allocation
// Expected (fixed): alloc_pages returns error

#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

use axalloc::{global_allocator, UsageKind};

fn poc_alloc_pages_overflow() {
    const PAGE_SIZE: usize = 4096;
    // num_pages * PAGE_SIZE overflows usize
    let num_pages = usize::MAX / PAGE_SIZE + 1;

    println!("=== Testing alloc_pages integer overflow ===");
    println!("num_pages: {}, overflowed size: {}", num_pages, num_pages.wrapping_mul(PAGE_SIZE));

    let result = global_allocator().alloc_pages(num_pages, PAGE_SIZE, UsageKind::RustHeap);

    match result {
        Ok(ptr) => {
            println!("BUG CONFIRMED: alloc_pages returned {:x} (undersized)", ptr);
            let _ = global_allocator().dealloc_pages(ptr, num_pages, UsageKind::RustHeap);
        }
        Err(e) => {
            println!("PASS: alloc_pages correctly rejected: {:?}", e);
        }
    }
}

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("PoC: Integer overflow in alloc_pages");
    poc_alloc_pages_overflow();
}
