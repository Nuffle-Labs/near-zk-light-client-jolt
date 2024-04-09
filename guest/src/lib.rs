#![cfg_attr(feature = "guest", no_std)]
#![no_main]

mod error;
//mod light_client;
mod merkle;

pub mod prelude {
    pub extern crate alloc;
    pub use alloc::*;
    pub use vec::Vec;
}

#[jolt::provable]
fn fib(n: u32) -> u128 {
    let mut a: u128 = 0;
    let mut b: u128 = 1;
    let mut sum: u128;
    for _ in 1..n {
        sum = a + b;
        a = b;
        b = sum;
    }

    b
}
