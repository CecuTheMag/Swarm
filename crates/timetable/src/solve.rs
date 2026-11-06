//! One search for a good timetable (spec 0013): a greedy start, then late acceptance
//! hill climbing (Burke and Bykov). A search is a pure function of the problem and its
//! [`Search`], so two devices running the same unit return the same timetable.

use crate::{
    cost::{HARD_WEIGHT, Tally},
    problem::Problem,
    rng::Rng,
};

/// History lengths the units of a job cycle through, a portfolio of search styles. Chosen
/// by measurement (docs/measurements.md): at the budgets a unit has (millions of
/// iterations), lengths from 10 to 100 do best; longer ones have no time to settle.
pub const HISTORY_LENGTHS: [u32; 4] = [10, 20, 50, 100];

/// One unit's search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Search {
    pub seed: u64,
    pub iterations: u32,
    /// How many steps back a candidate may be compared with (`L`).
    pub history: u32,
}

impl Search {
    /// The search of unit `index` of a job with `seed`: its own random numbers, and the
    /// portfolio's history lengths in turn. Every device derives it the same way.
    pub fn unit(seed: u32, index: u32, iterations: u32) -> Self {
        Search {
            seed: u64::from(seed) << 32 | u64::from(index),
            iterations,
            history: HISTORY_LENGTHS[index as usize % HISTORY_LENGTHS.len()],
        }
    }
}

/// An hour moving to `to`, from `from` (none when it is placed for the first time).
#[derive(Debug, Clone, Copy)]
struct Move {
    hour: usize,
    from: Option<usize>,
    to: usize,
}

/// The parts of the cost a set of moves can change.
#[derive(Debug, Default)]
struct Keys {
    class_days: Vec<(usize, usize)>,
    teacher_days: Vec<(usize, usize)>,
    lesson_days: Vec<(usize, usize)>,
    class_slots: Vec<(usize, usize)>,
    teacher_slots: Vec<(usize, usize)>,
    room_slots: Vec<(usize, usize)>,
}

fn push<T: PartialEq>(list: &mut Vec<T>, item: T) {
    if !list.contains(&item) {
        list.push(item);
    }
}

impl Keys {
    fn clear(&mut self) {
        self.class_days.clear();
        self.teacher_days.clear();
        self.lesson_days.clear();
        self.class_slots.clear();
        self.teacher_slots.clear();
        self.room_slots.clear();
    }
}

/// A timetable being searched, with its counts and its cost kept up to date.
pub(crate) struct State<'a> {
    p: &'a Problem,
    tally: Tally,
    /// The slot of each hour; `UNPLACED` before the greedy start places it.
    pub(crate) slots: Vec<u16>,
    pub(crate) cost: u64,
    keys: Keys,
}

const UNPLACED: u16 = u16::MAX;

impl<'a> State<'a> {
    pub(crate) fn new(p: &'a Problem) -> Self {
        State {
            p,
            tally: Tally::new(p),
            slots: vec![UNPLACED; p.hours.len()],
            cost: 0,
            keys: Keys::default(),
        }
    }

    fn collect(&mut self, moves: &[Move]) {
        let (p, k) = (self.p, &mut self.keys);
        k.clear();
        for m in moves {
            let hour = p.hours[m.hour];
            let (c, t, l) = (
                usize::from(hour.class),
                usize::from(hour.teacher),
                usize::from(hour.lesson),
            );
            for s in m.from.into_iter().chain([m.to]) {
                let d = s / p.periods;
                push(&mut k.class_days, (c, d));
                push(&mut k.teacher_days, (t, d));
                push(&mut k.lesson_days, (l, d));
                push(&mut k.class_slots, (c, s));
                push(&mut k.teacher_slots, (t, s));
                if let Some(r) = hour.room {
                    push(&mut k.room_slots, (usize::from(r), s));
                }
            }
        }
    }

    /// The cost of everything the collected keys cover.
    fn local(&self) -> u64 {
        let (p, t, k) = (self.p, &self.tally, &self.keys);
        k.class_days
            .iter()
            .map(|&(c, d)| t.class_day_cost(p, c, d))
            .chain(
                k.teacher_days
                    .iter()
                    .map(|&(x, d)| t.teacher_day_cost(p, x, d)),
            )
            .chain(
                k.lesson_days
                    .iter()
                    .map(|&(l, d)| t.lesson_day_cost(p, l, d)),
            )
            .chain(k.class_slots.iter().map(|&(c, s)| t.class_clash_cost(c, s)))
            .chain(
                k.teacher_slots
                    .iter()
                    .map(|&(x, s)| t.teacher_clash_cost(x, s)),
            )
            .chain(k.room_slots.iter().map(|&(r, s)| t.room_clash_cost(r, s)))
            .sum()
    }

    /// The hard cost of hour `h` at `slot` itself: its teacher or class unavailable
    /// there, or the hour away from its pin.
    fn placement(&self, h: usize, slot: usize) -> u64 {
        let (p, hour) = (self.p, self.p.hours[h]);
        let s = p.slots();
        (u64::from(p.teacher_off[usize::from(hour.teacher) * s + slot])
            + u64::from(p.class_off[usize::from(hour.class) * s + slot])
            + u64::from(hour.pin.is_some_and(|pin| usize::from(pin) != slot)))
            * HARD_WEIGHT
    }

    fn apply(&mut self, moves: &[Move]) {
        for m in moves {
            if let Some(from) = m.from {
                self.tally.remove(self.p, m.hour, from);
            }
            self.tally.add(self.p, m.hour, m.to);
            self.slots[m.hour] = m.to as u16;
        }
    }

    fn revert(&mut self, moves: &[Move]) {
        for m in moves.iter().rev() {
            self.tally.remove(self.p, m.hour, m.to);
            match m.from {
                Some(from) => {
                    self.tally.add(self.p, m.hour, from);
                    self.slots[m.hour] = from as u16;
                }
                None => self.slots[m.hour] = UNPLACED,
            }
        }
    }

    /// Applies `moves` and returns the change in cost; `revert` undoes them.
    fn attempt(&mut self, moves: &[Move]) -> i64 {
        self.collect(moves);
        let mut before = self.local();
        for m in moves {
            if let Some(from) = m.from {
                before += self.placement(m.hour, from);
            }
        }
        self.apply(moves);
        let mut after = self.local();
        for m in moves {
            after += self.placement(m.hour, m.to);
        }
        after as i64 - before as i64
    }

    /// Keeps the moves just attempted, and their change in cost.
    pub(crate) fn commit(&mut self, delta: i64) {
        self.cost = (self.cost as i64 + delta) as u64;
    }

    /// Moves hour `h` to `slot` (or places it) and updates the cost.
    pub(crate) fn place(&mut self, h: usize, slot: usize) {
        let from = (self.slots[h] != UNPLACED).then(|| usize::from(self.slots[h]));
        let m = [Move {
            hour: h,
            from,
            to: slot,
        }];
        let delta = self.attempt(&m);
        self.commit(delta);
    }

    /// Places pinned hours at their pins, then every other hour in turn at the slot
    /// where it costs least, in a random order; ties go to the first slot from a random
    /// starting point.
    fn greedy(&mut self, rng: &mut Rng) {
        for (h, hour) in self.p.hours.iter().enumerate() {
            if let Some(pin) = hour.pin {
                self.place(h, usize::from(pin));
            }
        }
        let mut order: Vec<usize> = self.p.free.iter().map(|&h| h as usize).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, rng.below(i + 1));
        }
        let slots = self.p.slots();
        for h in order {
            let offset = rng.below(slots);
            let mut best = (i64::MAX, 0);
            for i in 0..slots {
                let s = (offset + i) % slots;
                let m = [Move {
                    hour: h,
                    from: None,
                    to: s,
                }];
                let delta = self.attempt(&m);
                self.revert(&m);
                if delta < best.0 {
                    best = (delta, s);
                }
            }
            self.place(h, best.1);
        }
    }
}

/// Searches for a good timetable and returns the best one it found: one slot per hour,
/// in lesson order.
pub fn solve(problem: &Problem, search: Search) -> Vec<u16> {
    let mut rng = Rng::new(search.seed);
    let mut state = State::new(problem);
    state.greedy(&mut rng);

    // Only free hours move; with no pins they are every hour, in order, so a school
    // without pins draws the same random numbers as before pins existed.
    let free = &problem.free;
    let slots = problem.slots();
    let history_len = search.history.max(1) as usize;
    let mut history = vec![state.cost; history_len];
    let mut best_cost = state.cost;
    let mut best = state.slots.clone();
    // Whether the current timetable is the best so far: it is copied only when the
    // search is about to leave it, not on every improvement.
    let mut at_best = true;

    for i in 0..search.iterations as usize {
        if best_cost == 0 || free.is_empty() {
            break;
        }
        // Either one hour moves to another slot, or two hours of one class trade slots.
        let h1 = free[rng.below(free.len())] as usize;
        let s1 = usize::from(state.slots[h1]);
        let class = &problem.class_hours[usize::from(problem.hours[h1].class)];
        let mut buf = [Move {
            hour: h1,
            from: Some(s1),
            to: s1,
        }; 2];
        let n = if rng.below(2) == 0 && class.len() > 1 {
            let h2 = class[rng.below(class.len())] as usize;
            let s2 = usize::from(state.slots[h2]);
            if h2 == h1 || s2 == s1 {
                continue;
            }
            buf[0].to = s2;
            buf[1] = Move {
                hour: h2,
                from: Some(s2),
                to: s1,
            };
            2
        } else {
            buf[0].to = rng.below(slots);
            if buf[0].to == s1 {
                continue;
            }
            1
        };
        let moves = &buf[..n];

        let delta = state.attempt(moves);
        let candidate = (state.cost as i64 + delta) as u64;
        let v = i % history_len;
        if candidate <= state.cost || candidate <= history[v] {
            if at_best && candidate > best_cost {
                // Leaving the best timetable so far: keep a copy of it first.
                state.revert(moves);
                best.copy_from_slice(&state.slots);
                state.apply(moves);
                at_best = false;
            }
            state.commit(delta);
            if state.cost < best_cost {
                best_cost = state.cost;
                at_best = true;
            }
        } else {
            state.revert(moves);
        }
        history[v] = state.cost;
    }
    if at_best {
        best.copy_from_slice(&state.slots);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cost::cost, sample, testing::tiny};
    use proptest::prelude::*;

    #[test]
    fn the_tiny_school_gets_a_perfect_timetable() {
        let p = Problem::new(&tiny()).unwrap();
        let slots = solve(
            &p,
            Search {
                seed: 1,
                iterations: 10_000,
                history: 500,
            },
        );
        assert_eq!(cost(&p, &slots).unwrap().total, 0);
    }

    #[test]
    fn a_search_is_a_pure_function_of_its_inputs() {
        let p = Problem::new(&sample::school(3, 6)).unwrap();
        let search = Search {
            seed: 42,
            iterations: 20_000,
            history: 1_000,
        };
        assert_eq!(solve(&p, search), solve(&p, search));
        let other = solve(&p, Search { seed: 43, ..search });
        assert_ne!(solve(&p, search), other);
    }

    #[test]
    fn the_best_timetable_is_returned_with_its_true_cost() {
        let p = Problem::new(&sample::school(5, 10)).unwrap();
        let mut costs = Vec::new();
        for iterations in [0, 1_000, 30_000] {
            let slots = solve(
                &p,
                Search {
                    seed: 9,
                    iterations,
                    history: 500,
                },
            );
            assert!(slots.iter().all(|&s| usize::from(s) < p.slots()));
            costs.push(cost(&p, &slots).unwrap().total);
        }
        // More search never returns worse: the greedy start is where it begins.
        assert!(costs.windows(2).all(|w| w[1] <= w[0]), "{costs:?}");
    }

    /// An example school with some hours pinned at random slots: about one lesson in
    /// `one_in`, one or two hours each. Pins may clash or fall where a teacher is away.
    fn pinned_school(seed: u64, classes: usize, one_in: usize) -> crate::School {
        let mut school = sample::school(seed, classes);
        let mut rng = crate::rng::Rng::new(seed ^ 0x5eed);
        let (days, periods) = (usize::from(school.days), usize::from(school.periods));
        for l in &mut school.lessons {
            if rng.below(one_in) != 0 {
                continue;
            }
            for _ in 0..(1 + rng.below(2)).min(usize::from(l.per_week)) {
                let slot = crate::Slot {
                    day: rng.below(days) as u8,
                    period: rng.below(periods) as u8,
                };
                if !l.pinned.contains(&slot) {
                    l.pinned.push(slot);
                }
            }
        }
        school
    }

    #[test]
    fn a_school_with_every_hour_pinned_is_returned_as_pinned() {
        let mut school = tiny();
        school.lessons[0].pinned = vec![
            crate::Slot { day: 0, period: 0 },
            crate::Slot { day: 0, period: 1 },
            crate::Slot { day: 1, period: 0 },
        ];
        school.lessons[1].pinned = vec![
            crate::Slot { day: 1, period: 1 },
            crate::Slot { day: 0, period: 2 },
        ];
        let p = Problem::new(&school).unwrap();
        assert!(p.free.is_empty());
        let search = Search {
            seed: 3,
            iterations: 1_000,
            history: 10,
        };
        assert_eq!(solve(&p, search), vec![0, 1, 3, 4, 2]);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// No search moves a pinned hour, whatever the pins, and a search with pins is
        /// still a pure function of its inputs.
        #[test]
        fn pinned_hours_stay_where_they_are_pinned(
            seed in any::<u64>(),
            classes in 1usize..6,
            one_in in 1usize..6,
            iterations in 0u32..5_000,
        ) {
            let p = Problem::new(&pinned_school(seed, classes, one_in)).unwrap();
            let search = Search { seed, iterations, history: 20 };
            let slots = solve(&p, search);
            for (h, hour) in p.hours.iter().enumerate() {
                if let Some(pin) = hour.pin {
                    prop_assert_eq!(slots[h], pin);
                }
            }
            prop_assert_eq!(cost(&p, &slots).unwrap().breakdown.pins_moved, 0);
            prop_assert_eq!(solve(&p, search), slots);
        }

        /// The cost kept up to date move by move equals the cost from scratch, also when
        /// pinned hours are moved off their pins (as a person editing may).
        #[test]
        fn incremental_cost_equals_cost_from_scratch(
            seed in any::<u64>(),
            classes in 1usize..8,
            one_in in 1usize..8,
            moves in proptest::collection::vec((any::<u32>(), any::<u32>(), any::<bool>()), 1..200),
        ) {
            let p = Problem::new(&pinned_school(seed, classes, one_in)).unwrap();
            let mut state = State::new(&p);
            state.greedy(&mut crate::rng::Rng::new(seed));
            prop_assert_eq!(state.cost, cost(&p, &state.slots).unwrap().total);
            for (a, b, swap) in moves {
                let h1 = a as usize % p.hours.len();
                let s1 = usize::from(state.slots[h1]);
                let class = &p.class_hours[usize::from(p.hours[h1].class)];
                let ms: Vec<Move> = if swap && !class.is_empty() {
                    let h2 = class[b as usize % class.len()] as usize;
                    let s2 = usize::from(state.slots[h2]);
                    vec![
                        Move { hour: h1, from: Some(s1), to: s2 },
                        Move { hour: h2, from: Some(s2), to: s1 },
                    ]
                } else {
                    vec![Move { hour: h1, from: Some(s1), to: b as usize % p.slots() }]
                };
                let delta = state.attempt(&ms);
                if b % 3 == 0 {
                    state.revert(&ms);
                } else {
                    state.commit(delta);
                }
                prop_assert_eq!(state.cost, cost(&p, &state.slots).unwrap().total);
            }
        }
    }
}
