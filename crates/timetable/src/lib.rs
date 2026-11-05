//! Weekly school timetables (spec 0013).
//!
//! A [`School`] lists teachers, classes, rooms and lessons. [`Problem`] checks it and
//! expands every lesson into its weekly hours. A timetable gives each hour a slot (day ×
//! period); [`cost`] says how good it is, exactly and in integers, so every device and
//! the coordinator agree on it. [`solve`] searches for a good timetable from a seed: the
//! same inputs give the same timetable, bit for bit, natively and in WebAssembly.

mod cost;
mod model;
mod problem;
mod rng;
pub mod sample;
mod solve;

pub use cost::{Breakdown, Cost, HARD_WEIGHT, cost};
pub use model::{Class, Lesson, Room, School, Slot, Teacher};
pub use problem::{Hour, LIMITS, Limits, Problem};
pub use solve::{HISTORY_LENGTHS, Search, solve};

#[cfg(test)]
pub(crate) mod testing {
    use crate::model::{Class, Lesson, Room, School, Slot, Teacher};

    /// Two days of three periods; 8А has maths with Петрова (unavailable on day 1,
    /// period 2), 8Б has PE in the gym.
    pub fn tiny() -> School {
        School {
            name: "Малко училище".into(),
            days: 2,
            periods: 3,
            teachers: vec![
                Teacher {
                    name: "Петрова".into(),
                    unavailable: vec![Slot { day: 1, period: 2 }],
                    max_per_day: Some(3),
                },
                Teacher {
                    name: "Иванов".into(),
                    unavailable: vec![],
                    max_per_day: None,
                },
            ],
            classes: vec![
                Class {
                    name: "8А".into(),
                    unavailable: vec![],
                },
                Class {
                    name: "8Б".into(),
                    unavailable: vec![],
                },
            ],
            rooms: vec![Room {
                name: "Физкултурен салон".into(),
            }],
            lessons: vec![
                Lesson {
                    class: 0,
                    teacher: 0,
                    subject: "Математика".into(),
                    per_week: 3,
                    room: None,
                    max_per_day: 2,
                },
                Lesson {
                    class: 1,
                    teacher: 1,
                    subject: "Физическо".into(),
                    per_week: 2,
                    room: Some(0),
                    max_per_day: 1,
                },
            ],
        }
    }
}
