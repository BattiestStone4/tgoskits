// Bug: Integer overflow in page size calculation
// Severity: low
// Run: cargo xtask arceos qemu --package arceos-axalloc-integer-overflow-in-page-size --arch riscv64
// Expected: Demonstrates overflow in size calculation

#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

use axalloc::{global_allocator, UsageKind};

fn poc_page_size_overflow() {
    const PAGE_SIZE: usize = 4096;
    let max_safe_pages = usize::MAX / PAGE_SIZE;
    let overflow_pages = max_safe_pages + 1;

    println!("=== Integer Overflow in Page Size Calculation ===");
    println!("Platform: {}-bit, PAGE_SIZE: {}", usize::BITS, PAGE_SIZE);
    println!("Overflow trigger: {} pages", overflow_pages);
    println!("Wrapped size: {} (should be > usize::MAX)", overflow_pages.wrapping_mul(PAGE_SIZE));

    // Try through real allocator API
    let test_pages = 1024 * 1024 * 4; // 16GB on 64-bit, likely to fail
    println!("Trying alloc_pages({})...", test_pages);

    match global_allocator().alloc_pages(test_pages, PAGE_SIZE, UsageKind::RustHeap) {
        Ok(addr) => {
            println!("Allocated at {:x}", addr);
            global_allocator().dealloc_pages(addr, test_pages, UsageKind::RustHeap);
        }
        Err(e) => {
            println!("Failed (expected): {:?}", e);
            println!("But usage stats may have been recorded with overflowed size!");
        }
    }
}

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("PoC: Integer overflow in page size calculation");
    poc_page_size_overflow();
}
