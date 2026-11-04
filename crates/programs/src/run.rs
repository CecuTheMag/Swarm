//! Runs programs natively, in an interpreter (wasmi), with the limits a browser applies.
//! The simulated worker computes program units with it, and tests use it as the
//! reference for what a module outputs.

use crate::{
    EXPORT_INPUT, EXPORT_MEMORY, EXPORT_OUTPUT, EXPORT_RUN, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES,
    Rejection, check,
};
use wasmi::{Config, Engine, Linker, Module, Store, TrapCode};

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Rejected(#[from] Rejection),
    #[error("the input is {0} bytes; at most {MAX_INPUT_BYTES} are allowed")]
    InputTooLarge(usize),
    #[error("the program ran longer than its budget allows")]
    OutOfTime,
    #[error("the program failed: {0}")]
    Trap(String),
    #[error("the program returned error code {0}")]
    Code(i32),
    #[error("the output is {0} bytes; at most {MAX_OUTPUT_BYTES} are allowed")]
    OutputTooLarge(usize),
    #[error("the program pointed outside its memory")]
    OutOfBounds,
}

fn trap(e: wasmi::Error) -> RunError {
    if e.as_trap_code() == Some(TrapCode::OutOfFuel) {
        RunError::OutOfTime
    } else {
        RunError::Trap(e.to_string())
    }
}

/// Runs `module` on `input` and returns its output. `fuel` bounds the work, roughly in
/// instructions, as the browser's time limit does. The module is checked first: the
/// interpreter runs only what a browser worker would.
pub fn run(module: &[u8], input: &[u8], fuel: u64) -> Result<Vec<u8>, RunError> {
    check(module)?;
    if input.len() > MAX_INPUT_BYTES {
        return Err(RunError::InputTooLarge(input.len()));
    }
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, module).map_err(|e| Rejection::Invalid(e.to_string()))?;
    let mut store = Store::new(&engine, ());
    store.set_fuel(fuel).map_err(trap)?;
    // Nothing is linked: the module has no imports to satisfy.
    let instance = Linker::<()>::new(&engine)
        .instantiate_and_start(&mut store, &module)
        .map_err(trap)?;
    let memory = instance
        .get_memory(&store, EXPORT_MEMORY)
        .ok_or(Rejection::Memory)?;
    let func = |name| {
        instance
            .get_typed_func::<(), i32>(&store, name)
            .map_err(|e| RunError::Trap(e.to_string()))
    };
    let (run, output) = (func(EXPORT_RUN)?, func(EXPORT_OUTPUT)?);
    let write_input = instance
        .get_typed_func::<i32, i32>(&store, EXPORT_INPUT)
        .map_err(|e| RunError::Trap(e.to_string()))?;

    let at = write_input
        .call(&mut store, input.len() as i32)
        .map_err(trap)?;
    memory
        .write(&mut store, at as u32 as usize, input)
        .map_err(|_| RunError::OutOfBounds)?;
    let len = run.call(&mut store, ()).map_err(trap)?;
    if len < 0 {
        return Err(RunError::Code(len));
    }
    let len = len as usize;
    if len > MAX_OUTPUT_BYTES {
        return Err(RunError::OutputTooLarge(len));
    }
    let at = output.call(&mut store, ()).map_err(trap)?;
    let mut out = vec![0; len];
    memory
        .read(&store, at as u32 as usize, &mut out)
        .map_err(|_| RunError::OutOfBounds)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{program, program_with, reverse};

    const FUEL: u64 = 10_000_000;

    #[test]
    fn runs_a_program() {
        assert_eq!(run(&reverse(), b"swarm", FUEL).unwrap(), b"mraws");
        assert_eq!(run(&reverse(), b"", FUEL).unwrap(), b"");
        let long: Vec<u8> = (0..=255).cycle().take(1000).collect();
        let mut expected = long.clone();
        expected.reverse();
        assert_eq!(run(&reverse(), &long, FUEL).unwrap(), expected);
    }

    #[test]
    fn an_endless_loop_runs_out_of_time() {
        let endless = program("(loop $forever (br $forever)) (i32.const 0)");
        assert!(matches!(run(&endless, b"", FUEL), Err(RunError::OutOfTime)));
    }

    #[test]
    fn failures_are_reported() {
        assert!(matches!(
            run(&program("unreachable"), b"", FUEL),
            Err(RunError::Trap(_))
        ));
        assert!(matches!(
            run(&program("(i32.const -3)"), b"", FUEL),
            Err(RunError::Code(-3))
        ));
        // A 1 MiB output claimed from a 64 KiB memory.
        let oversized = program_with("(memory (export \"memory\") 1 1)", "(i32.const 1048576)");
        assert!(matches!(
            run(&oversized, b"", FUEL),
            Err(RunError::OutputTooLarge(_))
        ));
        let out_of_bounds = program_with("(memory (export \"memory\") 1 1)", "(i32.const 65000)");
        assert!(matches!(
            run(&out_of_bounds, b"", FUEL),
            Err(RunError::OutOfBounds)
        ));
        assert!(matches!(
            run(&reverse(), &vec![0; MAX_INPUT_BYTES + 1], FUEL),
            Err(RunError::InputTooLarge(_))
        ));
        assert!(matches!(
            run(b"not wasm", b"", FUEL),
            Err(RunError::Rejected(_))
        ));
    }

    #[test]
    fn memory_cannot_grow_beyond_its_maximum() {
        // Grows by one page past a maximum of 2: memory.grow answers -1, as in a browser.
        let greedy = program_with(
            "(memory (export \"memory\") 1 2)",
            "(drop (memory.grow (i32.const 1))) (if (i32.ne (memory.grow (i32.const 1)) (i32.const -1)) (then unreachable)) (i32.const 0)",
        );
        assert_eq!(run(&greedy, b"", FUEL).unwrap(), b"");
    }
}
