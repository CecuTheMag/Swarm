//! A school as people describe it. Indices refer to the school's lists.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, rename = "TtSchool"))]
#[serde(deny_unknown_fields)]
pub struct School {
    pub name: String,
    /// Days a week.
    pub days: u8,
    /// Periods a day.
    pub periods: u8,
    pub teachers: Vec<Teacher>,
    /// The school's classes (паралелки).
    pub classes: Vec<Class>,
    /// Rooms that must not be double-booked: labs, the gym. A class's own room needs no
    /// entry: the class is never in two places anyway.
    #[serde(default)]
    pub rooms: Vec<Room>,
    pub lessons: Vec<Lesson>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, rename = "TtTeacher"))]
#[serde(deny_unknown_fields)]
pub struct Teacher {
    pub name: String,
    /// Slots the teacher cannot teach.
    #[serde(default)]
    pub unavailable: Vec<Slot>,
    /// Most periods the teacher should teach in a day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub max_per_day: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, rename = "TtClass"))]
#[serde(deny_unknown_fields)]
pub struct Class {
    pub name: String,
    /// Slots the class has no lessons (another activity, an afternoon off).
    #[serde(default)]
    pub unavailable: Vec<Slot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, rename = "TtRoom"))]
#[serde(deny_unknown_fields)]
pub struct Room {
    pub name: String,
}

/// A subject a class has with a teacher, `per_week` times a week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, rename = "TtLesson"))]
#[serde(deny_unknown_fields)]
pub struct Lesson {
    pub class: u16,
    pub teacher: u16,
    pub subject: String,
    pub per_week: u8,
    /// The room it needs, if it needs one of the school's rooms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub room: Option<u16>,
    /// Most hours of it a class should have in one day.
    #[serde(default = "one")]
    pub max_per_day: u8,
}

fn one() -> u8 {
    1
}

/// Day and period, from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, rename = "TtSlot"))]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub day: u8,
    pub period: u8,
}
