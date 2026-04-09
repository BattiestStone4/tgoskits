// Bug: Integer overflow in dealloc_pages causes incorrect deallocation
// Severity: high
// Run: cargo xtask arceos qemu --package arceos-axalloc-integer-overflow-in-dealloc-pages --arch riscv64
// Expected (bug): dealloc_pages proceeds with wrong size due to overflow
// Expected (fixed): panic or graceful rejection

#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

use axalloc::{global_allocator, UsageKind};

const PAGE_SIZE: usize = 4096;

fn poc_dealloc_overflow() {
    // Step 1: allocate one page to get a valid address
    println!("Allocating 1 page...");
    let pos = match global_allocator().alloc_pages(1, PAGE_SIZE, UsageKind::RustHeap) {
        Ok(p) => p,
        Err(e) => {
            println!("FAIL: alloc_pages failed: {:?}", e);
            return;
        }
    };
    println!("Allocated at {:x}", pos);

    // Step 2: num_pages that overflows when multiplied by PAGE_SIZE
    let num_pages = usize::MAX / PAGE_SIZE + 1;
    println!("Calling dealloc_pages(pos={:#x}, num_pages={})...", pos, num_pages);
    println!("num_pages * PAGE_SIZE (overflowed): {}", num_pages.wrapping_mul(PAGE_SIZE));

    // Step 3: trigger the bug — overflow in size calculation
    global_allocator().dealloc_pages(pos, num_pages, UsageKind::RustHeap);

    println!("BUG CONFIRMED: dealloc_pages accepted overflowed num_pages");
}

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("PoC: Integer overflow in dealloc_pages");
    poc_dealloc_overflow();
}
