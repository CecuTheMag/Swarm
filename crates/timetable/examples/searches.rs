//! Quality against the number of searches on an example school (spec 0013).
//!
//! `cargo run --release -p timetable --example searches -- CLASSES ITERATIONS SEARCHES
//! [HISTORY]` prints each search's cost and time, and the best so far: what a grid of
//! that many devices would return. HISTORY fixes `L` instead of cycling through the
//! portfolio.

use std::time::Instant;
use timetable::{HISTORY_LENGTHS, Problem, Search, cost, sample, solve};

fn main() {
    let args: Vec<u64> = std::env::args()
        .skip(1)
        .map(|a| a.parse().expect("numbers"))
        .collect();
    let (classes, iterations, searches, history) = match args[..] {
        [c, i, s] => (c as usize, i as u32, s as usize, None),
        [c, i, s, h] => (c as usize, i as u32, s as usize, Some(h as u32)),
        _ => (20, 2_000_000, 8, None),
    };
    let school = sample::school(2026, classes);
    let p = Problem::new(&school).expect("a valid school");
    println!(
        "{}: {} teachers, {} rooms, {} lesson hours, {} slots",
        school.name,
        p.teachers,
        p.rooms,
        p.hours.len(),
        p.slots()
    );
    let mut best = u64::MAX;
    for i in 0..searches {
        let started = Instant::now();
        let search = Search {
            seed: 2026 << 32 | i as u64,
            iterations,
            history: history.unwrap_or(HISTORY_LENGTHS[i % HISTORY_LENGTHS.len()]),
        };
        let c = cost(&p, &solve(&p, search)).expect("a timetable");
        best = best.min(c.total);
        println!(
            "search {i:>3} (L = {:>4}): cost {:>9} (hard {}, soft {:>6}) in {:>5.2} s  best so far {best}",
            search.history,
            c.total,
            c.hard,
            c.soft,
            started.elapsed().as_secs_f64()
        );
    }
}
