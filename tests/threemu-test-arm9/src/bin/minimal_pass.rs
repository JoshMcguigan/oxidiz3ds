//! Minimal pass test for ARM9
//!
//! Simple test that immediately signals pass.

#![no_std]
#![no_main]

use arm9_test_helpers::test_pass;

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    test_pass()
}
