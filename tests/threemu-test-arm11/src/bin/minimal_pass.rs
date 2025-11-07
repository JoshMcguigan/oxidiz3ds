//! Minimal pass test for ARM11
//!
//! Simple test that immediately signals pass.

#![no_std]
#![no_main]

use arm11_test_helpers::test_pass;

#[unsafe(no_mangle)]
pub extern "C" fn main() -> ! {
    test_pass()
}
