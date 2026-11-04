//! Small programs for tests, in Rust and in the browser (spec 0008). The browser tests
//! load the compiled modules from `web/src/lib/program/fixtures`, which
//! `web_fixtures_are_current` keeps up to date.

/// FNV-1a (64-bit) of the input, as 8 little-endian bytes: every output byte depends on
/// every input byte.
pub fn fnv() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (memory (export "memory") 8 8)
            (global $len (mut i32) (i32.const 0))
            (func (export "swarm_input") (param i32) (result i32)
                (global.set $len (local.get 0))
                (i32.const 1024))
            (func (export "swarm_output") (result i32) (i32.const 512))
            (func (export "swarm_run") (result i32)
                (local $i i32) (local $h i64)
                (local.set $h (i64.const 0xcbf29ce484222325))
                (block $done (loop $next
                    (br_if $done (i32.ge_u (local.get $i) (global.get $len)))
                    (local.set $h (i64.mul
                        (i64.xor (local.get $h)
                                 (i64.extend_i32_u (i32.load8_u (i32.add (i32.const 1024) (local.get $i)))))
                        (i64.const 0x100000001b3)))
                    (local.set $i (i32.add (local.get $i) (i32.const 1)))
                    (br $next)))
                (i64.store (i32.const 512) (local.get $h))
                (i32.const 8)))"#,
    )
    .expect("valid text format")
}

/// What [`fnv`] computes, in Rust.
pub fn fnv_of(input: &[u8]) -> Vec<u8> {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in input {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h.to_le_bytes().to_vec()
}

/// Never returns.
pub fn endless() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (memory (export "memory") 1 1)
            (func (export "swarm_input") (param i32) (result i32) (i32.const 0))
            (func (export "swarm_output") (result i32) (i32.const 0))
            (func (export "swarm_run") (result i32) (loop $forever (br $forever)) (i32.const 0)))"#,
    )
    .expect("valid text format")
}

/// Fails every unit with a trap.
pub fn trap() -> Vec<u8> {
    wat::parse_str(
        r#"(module
            (memory (export "memory") 1 1)
            (func (export "swarm_input") (param i32) (result i32) (i32.const 0))
            (func (export "swarm_output") (result i32) (i32.const 0))
            (func (export "swarm_run") (result i32) unreachable))"#,
    )
    .expect("valid text format")
}

/// Grows its memory without a declared maximum: refused at upload.
pub fn unbounded() -> Vec<u8> {
    wat::parse_str(
        r#"(module (memory (export "memory") 1)
            (func (export "swarm_input") (param i32) (result i32) (i32.const 0))
            (func (export "swarm_run") (result i32) (i32.const 0))
            (func (export "swarm_output") (result i32) (i32.const 0)))"#,
    )
    .expect("valid text format")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixtures_behave_as_described() {
        assert!(crate::check(&fnv()).is_ok());
        assert!(crate::check(&endless()).is_ok());
        assert!(crate::check(&trap()).is_ok());
        assert_eq!(
            crate::check(&unbounded()),
            Err(crate::Rejection::NoMemoryMaximum)
        );
        #[cfg(feature = "run")]
        for input in [&b""[..], b"a", b"swarm", &[7; 5000]] {
            assert_eq!(
                crate::run(&fnv(), input, 10_000_000).unwrap(),
                fnv_of(input)
            );
        }
        // Known FNV-1a values.
        assert_eq!(fnv_of(b""), 0xcbf2_9ce4_8422_2325u64.to_le_bytes());
        assert_eq!(fnv_of(b"a"), 0xaf63_dc4c_8601_ec8cu64.to_le_bytes());
    }

    /// Writes the modules the browser tests load; CI fails if they differ from git.
    #[test]
    fn web_fixtures_are_current() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../web/src/lib/program/fixtures");
        std::fs::create_dir_all(&dir).unwrap();
        for (name, module) in [("fnv", fnv()), ("endless", endless()), ("trap", trap())] {
            std::fs::write(dir.join(format!("{name}.wasm")), module).unwrap();
        }
    }
}
