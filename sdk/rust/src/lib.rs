//! Write a program for Swarm in Rust.
//!
//! A program is a pure function from input bytes to output bytes. Swarm runs it on
//! volunteers' devices, one unit per input, compiled to WebAssembly:
//!
//! ```
//! /// Unit `i` counts the primes in `[i·10⁶, (i+1)·10⁶)`.
//! fn count_primes(input: &[u8]) -> Vec<u8> {
//!     let i = u32::from_le_bytes(input[..4].try_into().unwrap());
//!     # let _ = i;
//!     // …
//!     # let count = 0u32;
//!     count.to_le_bytes().to_vec()
//! }
//!
//! swarm_program::program!(count_primes);
//! ```
//!
//! Build it with `cargo build --release --target wasm32-unknown-unknown`, as a crate with
//! `crate-type = ["cdylib"]`, and declare how much memory it may use: Swarm accepts at
//! most 256 MiB, and needs the limit stated. In `.cargo/config.toml`:
//!
//! ```toml
//! [target.wasm32-unknown-unknown]
//! rustflags = ["-C", "link-arg=--max-memory=67108864"] # 64 MiB
//! ```
//!
//! A program cannot import anything, so it has no clock, randomness, files or network:
//! the same input always gives the same output, on every device. That is how Swarm
//! checks results, by comparing what different devices computed. Seed any pseudo-random
//! numbers from the input. A panic fails the unit, which then goes to another device.

use std::cell::RefCell;

thread_local! {
    static INPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Exports `run` as the program. `run` takes the unit's input and returns its output.
#[macro_export]
macro_rules! program {
    ($run:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn swarm_input(len: i32) -> i32 {
            $crate::__input(len)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn swarm_run() -> i32 {
            $crate::__run($run)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn swarm_output() -> i32 {
            $crate::__output()
        }
    };
}

// The functions below are what the exports call; they are public only for the macro.
// Pointers fit in an i32 on wasm32, the only target the exports are called on.

#[doc(hidden)]
pub fn __input(len: i32) -> i32 {
    INPUT.with_borrow_mut(|input| {
        input.clear();
        input.resize(len as u32 as usize, 0);
        input.as_mut_ptr() as usize as i32
    })
}

#[doc(hidden)]
pub fn __run(run: impl FnOnce(&[u8]) -> Vec<u8>) -> i32 {
    let output = INPUT.with_borrow(|input| run(input));
    OUTPUT.with_borrow_mut(|o| {
        *o = output;
        o.len() as i32
    })
}

#[doc(hidden)]
pub fn __output() -> i32 {
    OUTPUT.with_borrow(|o| o.as_ptr() as usize as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_runs_through_the_program() {
        __input(3);
        INPUT.with_borrow_mut(|i| i.copy_from_slice(b"abc"));
        let len = __run(|input| input.iter().rev().copied().collect());
        assert_eq!(len, 3);
        assert_eq!(OUTPUT.with_borrow(|o| o.clone()), b"cba");
        // The next unit starts from a fresh input.
        __input(0);
        assert_eq!(__run(|input| input.to_vec()), 0);
    }
}
