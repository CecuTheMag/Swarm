//! What a timetable costs (spec 0013). Lower is better; a timetable without hard
//! violations is valid. The same counts serve the search, which updates them as hours
//! move, and the coordinator, which recomputes them for every timetable a device sends.

use crate::problem::Problem;
use serde::{Deserialize, Serialize};

/// A hard violation outweighs any amount of soft ones in a real school.
pub const HARD_WEIGHT: u64 = 1_000_000;

const CLASS_GAP: u64 = 100;
const LATE_START: u64 = 30;
const SUBJECT_EXCESS: u64 = 50;
const TEACHER_EXCESS: u64 = 20;
const TEACHER_GAP: u64 = 5;
const CLASS_OVERLOAD: u64 = 20;

/// How many times each rule is broken.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, rename = "TtBreakdown"))]
pub struct Breakdown {
    /// Extra lessons of a teacher, class or room at one slot.
    pub clashes: u32,
    /// Lessons at a slot their teacher or class is unavailable.
    pub unavailable: u32,
    /// Empty periods inside a class's day.
    pub class_gaps: u32,
    /// Empty periods before a class's first lesson of the day.
    pub late_starts: u32,
    /// Hours of a subject beyond its most in a day.
    pub subject_excess: u32,
    /// Periods of a teacher beyond their most in a day.
    pub teacher_excess: u32,
    /// Empty periods inside a teacher's day.
    pub teacher_gaps: u32,
    /// Periods of a class beyond an even spread over the week.
    pub class_overload: u32,
}

impl Breakdown {
    pub fn hard(&self) -> u32 {
        self.clashes + self.unavailable
    }

    pub fn soft(&self) -> u64 {
        u64::from(self.class_gaps) * CLASS_GAP
            + u64::from(self.late_starts) * LATE_START
            + u64::from(self.subject_excess) * SUBJECT_EXCESS
            + u64::from(self.teacher_excess) * TEACHER_EXCESS
            + u64::from(self.teacher_gaps) * TEACHER_GAP
            + u64::from(self.class_overload) * CLASS_OVERLOAD
    }

    pub fn total(&self) -> u64 {
        u64::from(self.hard()) * HARD_WEIGHT + self.soft()
    }
}

/// A timetable's cost, with the rules it breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, rename = "TtCost"))]
pub struct Cost {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: u64,
    pub hard: u32,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub soft: u64,
    pub breakdown: Breakdown,
}

impl From<Breakdown> for Cost {
    fn from(breakdown: Breakdown) -> Self {
        Cost {
            total: breakdown.total(),
            hard: breakdown.hard(),
            soft: breakdown.soft(),
            breakdown,
        }
    }
}

/// The cost of `slots`, one slot per hour of `problem` in order, computed from scratch.
/// Refuses a timetable of the wrong length or with a slot outside the week.
pub fn cost(problem: &Problem, slots: &[u16]) -> Result<Cost, String> {
    if slots.len() != problem.hours.len() {
        return Err(format!(
            "{} slots for {} lesson hours",
            slots.len(),
            problem.hours.len()
        ));
    }
    if let Some(i) = slots
        .iter()
        .position(|&s| usize::from(s) >= problem.slots())
    {
        return Err(format!("hour {i}: slot {} is outside the week", slots[i]));
    }
    let mut tally = Tally::new(problem);
    for (h, &s) in slots.iter().enumerate() {
        tally.add(problem, h, usize::from(s));
    }
    Ok(tally.breakdown(problem, slots).into())
}

/// Counts of hours by entity and slot, from which every rule is evaluated. The search
/// keeps one up to date as hours move.
#[derive(Debug, Clone)]
pub(crate) struct Tally {
    slots: usize,
    periods: usize,
    pub(crate) class_at: Vec<u16>,
    pub(crate) teacher_at: Vec<u16>,
    pub(crate) room_at: Vec<u16>,
    /// Hours of each lesson on each day.
    pub(crate) lesson_day: Vec<u16>,
}

/// Extra items beyond the first at one place.
fn extra(n: u16) -> u64 {
    u64::from(n.saturating_sub(1))
}

/// Occupied periods, empty periods inside the day, and empty periods before the first
/// lesson, of one day of `counts` (one entry per period).
fn shape(counts: &[u16]) -> (u64, u64, u64) {
    let Some(first) = counts.iter().position(|&n| n > 0) else {
        return (0, 0, 0);
    };
    let last = counts
        .iter()
        .rposition(|&n| n > 0)
        .expect("one is occupied");
    let occupied = counts[first..=last].iter().filter(|&&n| n > 0).count();
    let span = last - first + 1;
    (occupied as u64, (span - occupied) as u64, first as u64)
}

impl Tally {
    pub(crate) fn new(p: &Problem) -> Self {
        let slots = p.slots();
        Tally {
            slots,
            periods: p.periods,
            class_at: vec![0; p.classes * slots],
            teacher_at: vec![0; p.teachers * slots],
            room_at: vec![0; p.rooms * slots],
            lesson_day: vec![0; p.lessons * p.days],
        }
    }

    pub(crate) fn add(&mut self, p: &Problem, h: usize, slot: usize) {
        let hour = p.hours[h];
        self.class_at[usize::from(hour.class) * self.slots + slot] += 1;
        self.teacher_at[usize::from(hour.teacher) * self.slots + slot] += 1;
        if let Some(r) = hour.room {
            self.room_at[usize::from(r) * self.slots + slot] += 1;
        }
        self.lesson_day[usize::from(hour.lesson) * p.days + slot / self.periods] += 1;
    }

    pub(crate) fn remove(&mut self, p: &Problem, h: usize, slot: usize) {
        let hour = p.hours[h];
        self.class_at[usize::from(hour.class) * self.slots + slot] -= 1;
        self.teacher_at[usize::from(hour.teacher) * self.slots + slot] -= 1;
        if let Some(r) = hour.room {
            self.room_at[usize::from(r) * self.slots + slot] -= 1;
        }
        self.lesson_day[usize::from(hour.lesson) * p.days + slot / self.periods] -= 1;
    }

    fn day<'a>(&self, at: &'a [u16], entity: usize, day: usize) -> &'a [u16] {
        let start = entity * self.slots + day * self.periods;
        &at[start..start + self.periods]
    }

    /// Gaps, late start and overload of a class's day.
    fn class_day(&self, p: &Problem, c: usize, d: usize) -> (u64, u64, u64) {
        let (occupied, gaps, late) = shape(self.day(&self.class_at, c, d));
        let over = occupied.saturating_sub(u64::from(p.class_cap[c]));
        (gaps, late, over)
    }

    /// Gaps and excess periods of a teacher's day.
    fn teacher_day(&self, p: &Problem, t: usize, d: usize) -> (u64, u64) {
        let (occupied, gaps, _) = shape(self.day(&self.teacher_at, t, d));
        (gaps, occupied.saturating_sub(u64::from(p.teacher_max[t])))
    }

    pub(crate) fn class_day_cost(&self, p: &Problem, c: usize, d: usize) -> u64 {
        let (gaps, late, over) = self.class_day(p, c, d);
        gaps * CLASS_GAP + late * LATE_START + over * CLASS_OVERLOAD
    }

    pub(crate) fn teacher_day_cost(&self, p: &Problem, t: usize, d: usize) -> u64 {
        let (gaps, excess) = self.teacher_day(p, t, d);
        gaps * TEACHER_GAP + excess * TEACHER_EXCESS
    }

    pub(crate) fn lesson_day_cost(&self, p: &Problem, l: usize, d: usize) -> u64 {
        u64::from(self.lesson_day[l * p.days + d].saturating_sub(u16::from(p.lesson_max[l])))
            * SUBJECT_EXCESS
    }

    pub(crate) fn class_clash_cost(&self, c: usize, slot: usize) -> u64 {
        extra(self.class_at[c * self.slots + slot]) * HARD_WEIGHT
    }

    pub(crate) fn teacher_clash_cost(&self, t: usize, slot: usize) -> u64 {
        extra(self.teacher_at[t * self.slots + slot]) * HARD_WEIGHT
    }

    pub(crate) fn room_clash_cost(&self, r: usize, slot: usize) -> u64 {
        extra(self.room_at[r * self.slots + slot]) * HARD_WEIGHT
    }

    /// Every rule over the whole timetable.
    fn breakdown(&self, p: &Problem, slots: &[u16]) -> Breakdown {
        let mut b = Breakdown::default();
        let clashes = |at: &[u16]| at.iter().map(|&n| extra(n)).sum::<u64>() as u32;
        b.clashes = clashes(&self.class_at) + clashes(&self.teacher_at) + clashes(&self.room_at);
        for (h, &s) in slots.iter().enumerate() {
            let (hour, s) = (p.hours[h], usize::from(s));
            b.unavailable += u32::from(p.teacher_off[usize::from(hour.teacher) * self.slots + s])
                + u32::from(p.class_off[usize::from(hour.class) * self.slots + s]);
        }
        for d in 0..p.days {
            for c in 0..p.classes {
                let (gaps, late, over) = self.class_day(p, c, d);
                b.class_gaps += gaps as u32;
                b.late_starts += late as u32;
                b.class_overload += over as u32;
            }
            for t in 0..p.teachers {
                let (gaps, excess) = self.teacher_day(p, t, d);
                b.teacher_gaps += gaps as u32;
                b.teacher_excess += excess as u32;
            }
            for l in 0..p.lessons {
                b.subject_excess += u32::from(
                    self.lesson_day[l * p.days + d].saturating_sub(u16::from(p.lesson_max[l])),
                );
            }
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::tiny;

    /// The slot of `day`, `period` in the tiny school (3 periods a day).
    fn at(day: u16, period: u16) -> u16 {
        day * 3 + period
    }

    #[test]
    fn a_clean_timetable_costs_nothing() {
        let p = Problem::new(&tiny()).unwrap();
        // 8А maths: day 0 periods 0–1 (max 2 a day), day 1 period 0. 8Б PE: periods 0
        // of both days.
        let slots = [at(0, 0), at(0, 1), at(1, 0), at(0, 0), at(1, 0)];
        let c = cost(&p, &slots).unwrap();
        assert_eq!(c.breakdown, Breakdown::default());
        assert_eq!(c.total, 0);
    }

    #[test]
    fn each_rule_is_counted() {
        let p = Problem::new(&tiny()).unwrap();
        // Two maths hours of 8А at one slot (a class and a teacher clash), the third at
        // day 1 period 2, when Петрова is unavailable. 8Б has both PE hours on day 0.
        let slots = [at(0, 0), at(0, 0), at(1, 2), at(0, 1), at(0, 2)];
        let c = cost(&p, &slots).unwrap();
        assert_eq!(
            c.breakdown,
            Breakdown {
                clashes: 2,
                unavailable: 1,
                // 8А starts day 1 at period 2, 8Б day 0 at period 1.
                late_starts: 3,
                // PE twice on day 0, at most once a day.
                subject_excess: 1,
                ..Breakdown::default()
            }
        );
        assert_eq!((c.hard, c.soft), (3, 3 * 30 + 50));
        assert_eq!(c.total, 3 * HARD_WEIGHT + 140);
    }

    #[test]
    fn gaps_and_overload() {
        let p = Problem::new(&tiny()).unwrap();
        // 8А: periods 0 and 2 of day 0 with a gap, third hour day 1 period 0.
        let slots = [at(0, 0), at(0, 2), at(1, 0), at(0, 0), at(1, 0)];
        let c = cost(&p, &slots).unwrap();
        assert_eq!(c.breakdown.class_gaps, 1);
        // Петрова teaches periods 0 and 2 of day 0: one gap.
        assert_eq!(c.breakdown.teacher_gaps, 1);
        assert_eq!(c.hard, 0);
        assert_eq!(c.soft, 100 + 5);
    }

    #[test]
    fn malformed_timetables_are_refused() {
        let p = Problem::new(&tiny()).unwrap();
        assert!(
            cost(&p, &[0, 1, 2, 3])
                .unwrap_err()
                .contains("4 slots for 5")
        );
        assert!(
            cost(&p, &[0, 1, 2, 3, 6])
                .unwrap_err()
                .contains("outside the week")
        );
    }
}
