//! Programs: WebAssembly modules that anyone may run on Swarm (spec 0008).
//!
//! A program is a module **without imports**: it cannot call anything outside itself,
//! so it has no network, no files, no clock and no randomness, only its own memory. That
//! is checked here from the module's structure, before any device sees it, and it is what
//! makes running a stranger's program on a contributor's device safe.
//!
//! The interface is four exports:
//!
//! | Export | Type | Meaning |
//! |---|---|---|
//! | `memory` | memory | the module's linear memory, with a declared maximum |
//! | `swarm_input` | `(len: i32) -> i32` | where the host writes the `len` input bytes |
//! | `swarm_run` | `() -> i32` | computes; the output's length, or a negative error code |
//! | `swarm_output` | `() -> i32` | where the output bytes are |

use wasmparser::{ValType, Validator, WasmFeatures, types::EntityType};

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures;
#[cfg(feature = "run")]
mod run;
#[cfg(feature = "run")]
pub use run::{RunError, run};

/// Largest module accepted.
pub const MAX_MODULE_BYTES: usize = 4 << 20;
/// Largest memory a module may declare: 256 MiB, in 64 KiB pages.
pub const MAX_MEMORY_PAGES: u64 = 4096;
/// Largest input and output of one unit.
pub const MAX_INPUT_BYTES: usize = 256 << 10;
pub const MAX_OUTPUT_BYTES: usize = 256 << 10;

pub const EXPORT_MEMORY: &str = "memory";
pub const EXPORT_INPUT: &str = "swarm_input";
pub const EXPORT_RUN: &str = "swarm_run";
pub const EXPORT_OUTPUT: &str = "swarm_output";

/// What every current browser engine runs: WebAssembly 2.0, which includes fixed-width
/// SIMD. Left out on purpose: threads and shared memory (they need imports and
/// cross-origin isolation), relaxed SIMD (allowed to give different results on different
/// devices, which would break verification), 64-bit memory, exceptions, garbage
/// collection, tail calls and the rest of WebAssembly 3.0, which not every engine has.
pub fn features() -> WasmFeatures {
    WasmFeatures::WASM2
}

/// Why a module was refused, in words that say what to change.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    #[error("the module is {size} bytes; at most {MAX_MODULE_BYTES} are allowed")]
    TooLarge { size: usize },
    #[error("not a valid WebAssembly module, or it uses a feature Swarm does not allow: {0}")]
    Invalid(String),
    #[error(
        "the module imports {}; programs may not import anything (no host functions, \
         no WASI): build for wasm32-unknown-unknown and use no imports",
        .0.join(", ")
    )]
    Imports(Vec<String>),
    #[error("the module must define exactly one memory, exported as \"memory\"")]
    Memory,
    #[error(
        "the memory has no maximum size; declare one of at most {} MiB \
         (Rust: -C link-arg=--max-memory=N, C: -Wl,--max-memory=N)",
        MAX_MEMORY_PAGES / 16
    )]
    NoMemoryMaximum,
    #[error("the memory may grow to {} MiB; at most {} MiB are allowed", .pages / 16, MAX_MEMORY_PAGES / 16)]
    MemoryTooLarge { pages: u64 },
    #[error("missing export {name:?} of type {expected}")]
    MissingExport {
        name: &'static str,
        expected: &'static str,
    },
    #[error("export {name:?} must be of type {expected}")]
    WrongExport {
        name: &'static str,
        expected: &'static str,
    },
}

/// What a valid module declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checked {
    /// The memory's maximum, in 64 KiB pages.
    pub memory_max_pages: u64,
}

/// The functions a program exports, and their types as `(params, results)`.
const FUNCTIONS: [(&str, &[ValType], &[ValType], &str); 3] = [
    (
        EXPORT_INPUT,
        &[ValType::I32],
        &[ValType::I32],
        "(i32) -> i32",
    ),
    (EXPORT_RUN, &[], &[ValType::I32], "() -> i32"),
    (EXPORT_OUTPUT, &[], &[ValType::I32], "() -> i32"),
];

/// Checks that `bytes` is a module Swarm may run.
pub fn check(bytes: &[u8]) -> Result<Checked, Rejection> {
    if bytes.len() > MAX_MODULE_BYTES {
        return Err(Rejection::TooLarge { size: bytes.len() });
    }
    let types = Validator::new_with_features(features())
        .validate_all(bytes)
        .map_err(|e| Rejection::Invalid(e.message().to_owned()))?;
    let types = types.as_ref();

    let imports: Vec<String> = types
        .core_imports()
        .into_iter()
        .flatten()
        .map(|(module, name, _)| format!("{module}.{name}"))
        .collect();
    if !imports.is_empty() {
        return Err(Rejection::Imports(imports));
    }

    // Without imports, every memory is the module's own. Shared and 64-bit memories are
    // outside the allowed features, so the validator has refused them already.
    if types.memory_count() != 1 {
        return Err(Rejection::Memory);
    }
    let memory = types.memory_at(0);
    let pages = memory.maximum.ok_or(Rejection::NoMemoryMaximum)?;
    if pages > MAX_MEMORY_PAGES {
        return Err(Rejection::MemoryTooLarge { pages });
    }

    let exports: Vec<(&str, EntityType)> = types.core_exports().into_iter().flatten().collect();
    let find = |name: &str| exports.iter().find(|(n, _)| *n == name).map(|(_, t)| *t);
    match find(EXPORT_MEMORY) {
        Some(EntityType::Memory(_)) => {}
        Some(_) => {
            return Err(Rejection::WrongExport {
                name: EXPORT_MEMORY,
                expected: "memory",
            });
        }
        None => return Err(Rejection::Memory),
    }
    for (name, params, results, expected) in FUNCTIONS {
        let id = match find(name) {
            Some(EntityType::Func(id) | EntityType::FuncExact(id)) => id,
            Some(_) => return Err(Rejection::WrongExport { name, expected }),
            None => return Err(Rejection::MissingExport { name, expected }),
        };
        let ty = types[id].unwrap_func();
        if ty.params() != params || ty.results() != results {
            return Err(Rejection::WrongExport { name, expected });
        }
    }
    Ok(Checked {
        memory_max_pages: pages,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A complete program around `body`, the instructions of `swarm_run`. Input goes at
    /// offset 1024, output at 2048; `$len` holds the input's length.
    pub fn program(body: &str) -> Vec<u8> {
        program_with("(memory (export \"memory\") 1 16)", body)
    }

    pub fn program_with(memory: &str, body: &str) -> Vec<u8> {
        wat::parse_str(format!(
            r#"(module
                {memory}
                (global $len (mut i32) (i32.const 0))
                (func (export "swarm_input") (param i32) (result i32)
                    (global.set $len (local.get 0))
                    (i32.const 1024))
                (func (export "swarm_output") (result i32) (i32.const 2048))
                (func (export "swarm_run") (result i32) {body}))"#
        ))
        .unwrap()
    }

    /// Copies the input to the output, reversed.
    pub fn reverse() -> Vec<u8> {
        program(
            r#"(local $i i32)
               (block $done (loop $next
                   (br_if $done (i32.ge_u (local.get $i) (global.get $len)))
                   (i32.store8
                       (i32.add (i32.const 2048) (local.get $i))
                       (i32.load8_u (i32.sub (i32.add (i32.const 1023) (global.get $len))
                                             (local.get $i))))
                   (local.set $i (i32.add (local.get $i) (i32.const 1)))
                   (br $next)))
               (global.get $len)"#,
        )
    }

    #[test]
    fn a_minimal_program_is_accepted() {
        assert_eq!(
            check(&reverse()),
            Ok(Checked {
                memory_max_pages: 16
            })
        );
        let largest = program_with("(memory (export \"memory\") 1 4096)", "(i32.const 0)");
        assert!(check(&largest).is_ok());
    }

    #[test]
    fn imports_are_refused() {
        let module = wat::parse_str(
            r#"(module
                (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
                (import "env" "fetch" (func))
                (memory (export "memory") 1 1)
                (func (export "swarm_input") (param i32) (result i32) (i32.const 0))
                (func (export "swarm_run") (result i32) (i32.const 0))
                (func (export "swarm_output") (result i32) (i32.const 0)))"#,
        )
        .unwrap();
        assert_eq!(
            check(&module),
            Err(Rejection::Imports(vec![
                "wasi_snapshot_preview1.fd_write".into(),
                "env.fetch".into()
            ]))
        );

        let imported_memory = wat::parse_str(
            r#"(module (import "env" "memory" (memory 1 1)) (export "memory" (memory 0)))"#,
        )
        .unwrap();
        assert!(matches!(
            check(&imported_memory),
            Err(Rejection::Imports(_))
        ));
    }

    #[test]
    fn memory_needs_a_bounded_maximum() {
        let unbounded = program_with("(memory (export \"memory\") 1)", "(i32.const 0)");
        assert_eq!(check(&unbounded), Err(Rejection::NoMemoryMaximum));
        let huge = program_with("(memory (export \"memory\") 1 4097)", "(i32.const 0)");
        assert_eq!(check(&huge), Err(Rejection::MemoryTooLarge { pages: 4097 }));
        let hidden = program_with("(memory 1 1)", "(i32.const 0)");
        assert_eq!(check(&hidden), Err(Rejection::Memory));
        let none = program_with("", "(i32.const 0)");
        assert_eq!(check(&none), Err(Rejection::Memory));
    }

    #[test]
    fn exports_need_the_right_types() {
        let missing = wat::parse_str(
            r#"(module (memory (export "memory") 1 1)
                (func (export "swarm_input") (param i32) (result i32) (i32.const 0))
                (func (export "swarm_output") (result i32) (i32.const 0)))"#,
        )
        .unwrap();
        assert_eq!(
            check(&missing),
            Err(Rejection::MissingExport {
                name: EXPORT_RUN,
                expected: "() -> i32"
            })
        );
        let mistyped = wat::parse_str(
            r#"(module (memory (export "memory") 1 1)
                (func (export "swarm_input") (param i64) (result i32) (i32.const 0))
                (func (export "swarm_run") (result i32) (i32.const 0))
                (func (export "swarm_output") (result i32) (i32.const 0)))"#,
        )
        .unwrap();
        assert_eq!(
            check(&mistyped),
            Err(Rejection::WrongExport {
                name: EXPORT_INPUT,
                expected: "(i32) -> i32"
            })
        );
        let not_a_function = wat::parse_str(
            r#"(module (memory (export "memory") 1 1)
                (global (export "swarm_run") i32 (i32.const 0))
                (func (export "swarm_input") (param i32) (result i32) (i32.const 0))
                (func (export "swarm_output") (result i32) (i32.const 0)))"#,
        )
        .unwrap();
        assert!(matches!(
            check(&not_a_function),
            Err(Rejection::WrongExport {
                name: EXPORT_RUN,
                ..
            })
        ));
    }

    #[test]
    fn features_outside_the_allowed_set_are_refused() {
        // Shared memory (threads), 64-bit memory, relaxed SIMD and tail calls.
        for memory in [
            "(memory (export \"memory\") 1 1 shared)",
            "(memory (export \"memory\") i64 1 1)",
        ] {
            let module = program_with(memory, "(i32.const 0)");
            assert!(
                matches!(check(&module), Err(Rejection::Invalid(_))),
                "{memory}"
            );
        }
        for body in [
            "(drop (f32x4.relaxed_madd (v128.const i32x4 0 0 0 0) (v128.const i32x4 0 0 0 0) (v128.const i32x4 0 0 0 0))) (i32.const 0)",
            "(return_call 0 (i32.const 0))",
        ] {
            assert!(
                matches!(check(&program(body)), Err(Rejection::Invalid(_))),
                "{body}"
            );
        }
        // Fixed-width SIMD is fine.
        let simd = program("(i32x4.extract_lane 0 (i32x4.splat (i32.const 7)))");
        assert!(check(&simd).is_ok());
    }

    #[test]
    fn garbage_and_oversized_modules_are_refused() {
        assert!(matches!(check(b""), Err(Rejection::Invalid(_))));
        assert!(matches!(
            check(b"\0asm\x01\0\0\0junk"),
            Err(Rejection::Invalid(_))
        ));
        assert_eq!(
            check(&vec![0; MAX_MODULE_BYTES + 1]),
            Err(Rejection::TooLarge {
                size: MAX_MODULE_BYTES + 1
            })
        );
    }

    #[test]
    fn rejections_say_what_to_change() {
        assert!(
            Rejection::NoMemoryMaximum
                .to_string()
                .contains("--max-memory")
        );
        assert!(
            Rejection::Imports(vec!["env.f".into()])
                .to_string()
                .contains("wasm32-unknown-unknown")
        );
        assert!(
            Rejection::MemoryTooLarge { pages: 8192 }
                .to_string()
                .contains("512 MiB")
        );
    }
}
