// Bug: Potential re-entrancy/deadlock in tracking feature
// Severity: high
// Run: cargo xtask arceos qemu --package arceos-axalloc-potential-re-entrancy-deadlock-in --arch riscv64
// Expected (bug): deadlock or stack overflow from recursive alloc
// Expected (fixed): completes without issue

#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[macro_use]
#[cfg(feature = "axstd")]
extern crate axstd as std;

use axalloc::global_allocator;
use core::alloc::Layout;
use core::ptr::NonNull;

fn poc_tracking_reentrancy() {
    let alloc = global_allocator();
    println!("Stress test: mass allocation/deallocation");

    let mut ptrs: [Option<NonNull<u8>>; 512] = [None; 512];
    let mut layouts: [Option<Layout>; 512] = [None; 512];
    let mut count = 0;

    for i in 0..512 {
        let size = 16 + (i % 256);
        let layout = Layout::from_size_align(size, 8).unwrap();

        match alloc.alloc(layout) {
            Ok(ptr) => {
                ptrs[i] = Some(ptr);
                layouts[i] = Some(layout);
                count += 1;
            }
            Err(_) => {
                println!("Alloc failed at {}", i);
                break;
            }
        }

        // Interleaved dealloc
        if i >= 32 && i % 32 == 0 {
            if let (Some(nn), Some(l)) = (ptrs[i - 16], layouts[i - 16]) {
                unsafe { alloc.dealloc(nn, l) };
                ptrs[i - 16] = None;
            }
        }
    }

    println!("Phase 1: {} allocations", count);

    // Cleanup
    for i in 0..512 {
        if let (Some(nn), Some(l)) = (ptrs[i], layouts[i]) {
            unsafe { alloc.dealloc(nn, l) };
        }
    }
    println!("Cleanup done");
}

#[cfg_attr(feature = "axstd", unsafe(no_mangle))]
fn main() {
    println!("PoC: Re-entrancy/deadlock in tracking");
    poc_tracking_reentrancy();
    println!("Completed (tracking disabled or has protection)");
}
