//! A school checked and expanded into the hours a timetable places.

use crate::model::{School, Slot};

/// How large a school may be: enough for the largest Bulgarian schools, small enough
/// that a phone searches it in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub days: u8,
    pub periods: u8,
    pub teachers: usize,
    pub classes: usize,
    pub rooms: usize,
    pub lessons: usize,
    pub hours: usize,
    pub name_chars: usize,
}

pub const LIMITS: Limits = Limits {
    days: 7,
    periods: 12,
    teachers: 200,
    classes: 100,
    rooms: 100,
    lessons: 2000,
    hours: 4000,
    name_chars: 80,
};

/// One weekly hour of a lesson.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hour {
    pub lesson: u16,
    pub class: u16,
    pub teacher: u16,
    pub room: Option<u16>,
    /// The slot it is pinned at (spec 0015), if it is.
    pub pin: Option<u16>,
}

/// A school, checked, with its hours in lesson order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub days: usize,
    pub periods: usize,
    pub teachers: usize,
    pub classes: usize,
    pub rooms: usize,
    pub hours: Vec<Hour>,
    /// Hours that are not pinned: the ones a search moves.
    pub free: Vec<u32>,
    /// Free hours of each class, for moves that swap two of them.
    pub class_hours: Vec<Vec<u32>>,
    /// `max_per_day` of each lesson.
    pub lesson_max: Vec<u8>,
    /// Lessons of the school, for counting a subject's hours in a day.
    pub lessons: usize,
    /// Each teacher's most periods a day; `u8::MAX` for no limit.
    pub teacher_max: Vec<u8>,
    /// Most periods a class's day should hold for an even spread over the week.
    pub class_cap: Vec<u8>,
    /// Unavailable slots by teacher and by class, `slots` entries each.
    pub teacher_off: Vec<bool>,
    pub class_off: Vec<bool>,
}

fn check_name(what: &str, name: &str) -> Result<(), String> {
    let chars = name.chars().count();
    if name.trim().is_empty() || chars > LIMITS.name_chars || name.chars().any(char::is_control) {
        return Err(format!(
            "{what}: a name of 1 to {} characters is needed",
            LIMITS.name_chars
        ));
    }
    Ok(())
}

/// Refuses two entries of one list with the same name, ignoring case and surrounding
/// spaces: a printed timetable that says „Иванова“ must mean one person.
fn check_unique<'a>(list: &str, names: impl Iterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = std::collections::HashMap::new();
    for (i, name) in names.enumerate() {
        if let Some(first) = seen.insert(name.trim().to_lowercase(), i) {
            return Err(format!("{list} {first} and {i} have the same name: {name}"));
        }
    }
    Ok(())
}

impl Problem {
    pub fn slots(&self) -> usize {
        self.days * self.periods
    }

    pub fn slot_of(&self, slot: usize) -> Slot {
        Slot {
            day: (slot / self.periods) as u8,
            period: (slot % self.periods) as u8,
        }
    }

    /// Checks `school` and expands its lessons into hours.
    pub fn new(school: &School) -> Result<Self, String> {
        check_name("the school", &school.name)?;
        if !(1..=LIMITS.days).contains(&school.days) {
            return Err(format!("days must be between 1 and {}", LIMITS.days));
        }
        if !(1..=LIMITS.periods).contains(&school.periods) {
            return Err(format!("periods must be between 1 and {}", LIMITS.periods));
        }
        for (list, len, max) in [
            ("teachers", school.teachers.len(), LIMITS.teachers),
            ("classes", school.classes.len(), LIMITS.classes),
            ("rooms", school.rooms.len(), LIMITS.rooms),
            ("lessons", school.lessons.len(), LIMITS.lessons),
        ] {
            if len > max {
                return Err(format!("at most {max} {list}"));
            }
        }
        if school.lessons.is_empty() {
            return Err("a school needs at least one lesson".into());
        }
        let (days, periods) = (usize::from(school.days), usize::from(school.periods));
        let slots = days * periods;
        let in_range = |what: &str, i: usize, s: &Slot| {
            if s.day >= school.days || s.period >= school.periods {
                return Err(format!(
                    "{what} {i}: unavailable slot day {} period {} is outside the week",
                    s.day, s.period
                ));
            }
            Ok(usize::from(s.day) * periods + usize::from(s.period))
        };

        let mut teacher_off = vec![false; school.teachers.len() * slots];
        let mut teacher_max = Vec::with_capacity(school.teachers.len());
        for (i, t) in school.teachers.iter().enumerate() {
            check_name(&format!("teacher {i}"), &t.name)?;
            for s in &t.unavailable {
                teacher_off[i * slots + in_range("teacher", i, s)?] = true;
            }
            match t.max_per_day {
                Some(0) => return Err(format!("teacher {i}: max_per_day must be at least 1")),
                Some(m) => teacher_max.push(m),
                None => teacher_max.push(u8::MAX),
            }
        }
        let mut class_off = vec![false; school.classes.len() * slots];
        for (i, c) in school.classes.iter().enumerate() {
            check_name(&format!("class {i}"), &c.name)?;
            for s in &c.unavailable {
                class_off[i * slots + in_range("class", i, s)?] = true;
            }
        }
        for (i, r) in school.rooms.iter().enumerate() {
            check_name(&format!("room {i}"), &r.name)?;
        }
        check_unique("teachers", school.teachers.iter().map(|t| t.name.as_str()))?;
        check_unique("classes", school.classes.iter().map(|c| c.name.as_str()))?;
        check_unique("rooms", school.rooms.iter().map(|r| r.name.as_str()))?;

        let mut hours = Vec::new();
        let mut free = Vec::new();
        let mut class_hours = vec![Vec::new(); school.classes.len()];
        let mut class_total = vec![0usize; school.classes.len()];
        let mut lesson_max = Vec::with_capacity(school.lessons.len());
        for (i, l) in school.lessons.iter().enumerate() {
            check_name(&format!("lesson {i}"), &l.subject)?;
            if usize::from(l.class) >= school.classes.len() {
                return Err(format!("lesson {i}: no class {}", l.class));
            }
            if usize::from(l.teacher) >= school.teachers.len() {
                return Err(format!("lesson {i}: no teacher {}", l.teacher));
            }
            if let Some(r) = l.room
                && usize::from(r) >= school.rooms.len()
            {
                return Err(format!("lesson {i}: no room {r}"));
            }
            if l.per_week == 0 {
                return Err(format!("lesson {i}: per_week must be at least 1"));
            }
            if l.max_per_day == 0 {
                return Err(format!("lesson {i}: max_per_day must be at least 1"));
            }
            if l.pinned.len() > usize::from(l.per_week) {
                return Err(format!(
                    "lesson {i}: {} hours pinned but {} a week",
                    l.pinned.len(),
                    l.per_week
                ));
            }
            let mut pins = Vec::with_capacity(l.pinned.len());
            for s in &l.pinned {
                if s.day >= school.days || s.period >= school.periods {
                    return Err(format!(
                        "lesson {i}: pinned slot day {} period {} is outside the week",
                        s.day, s.period
                    ));
                }
                let slot = (usize::from(s.day) * periods + usize::from(s.period)) as u16;
                if pins.contains(&slot) {
                    return Err(format!(
                        "lesson {i}: slot day {} period {} is pinned twice",
                        s.day, s.period
                    ));
                }
                pins.push(slot);
            }
            lesson_max.push(l.max_per_day);
            for k in 0..usize::from(l.per_week) {
                if hours.len() == LIMITS.hours {
                    return Err(format!("at most {} lesson hours a week", LIMITS.hours));
                }
                let pin = pins.get(k).copied();
                if pin.is_none() {
                    free.push(hours.len() as u32);
                    class_hours[usize::from(l.class)].push(hours.len() as u32);
                }
                class_total[usize::from(l.class)] += 1;
                hours.push(Hour {
                    lesson: i as u16,
                    class: l.class,
                    teacher: l.teacher,
                    room: l.room,
                    pin,
                });
            }
        }
        let class_cap = class_total
            .iter()
            .map(|&n| (n.div_ceil(days) + 1).min(periods) as u8)
            .collect();
        Ok(Problem {
            days,
            periods,
            teachers: school.teachers.len(),
            classes: school.classes.len(),
            rooms: school.rooms.len(),
            hours,
            free,
            class_hours,
            lesson_max,
            lessons: school.lessons.len(),
            teacher_max,
            class_cap,
            teacher_off,
            class_off,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::tiny;

    #[test]
    fn lessons_expand_into_hours() {
        let p = Problem::new(&tiny()).unwrap();
        assert_eq!(p.slots(), 6);
        assert_eq!(p.hours.len(), 5);
        assert_eq!(p.class_hours, vec![vec![0, 1, 2], vec![3, 4]]);
        assert_eq!(p.free, vec![0, 1, 2, 3, 4]);
        assert_eq!(p.hours[3].room, Some(0));
        assert!(p.hours.iter().all(|h| h.pin.is_none()));
        assert_eq!(p.teacher_max, vec![3, u8::MAX]);
        // 3 hours over 2 days: at most 2 a day, plus one of slack.
        assert_eq!(p.class_cap, vec![3, 2]);
        assert!(p.teacher_off[5]);
        assert_eq!(p.slot_of(4), Slot { day: 1, period: 1 });
    }

    #[test]
    fn pinned_hours_are_the_first_of_their_lesson() {
        let mut school = tiny();
        school.lessons[0].pinned = vec![Slot { day: 1, period: 0 }, Slot { day: 0, period: 2 }];
        let p = Problem::new(&school).unwrap();
        let pins: Vec<_> = p.hours.iter().map(|h| h.pin).collect();
        assert_eq!(pins, vec![Some(3), Some(2), None, None, None]);
        assert_eq!(p.free, vec![2, 3, 4]);
        // Only free hours are swapped; the even spread still counts every hour.
        assert_eq!(p.class_hours, vec![vec![2], vec![3, 4]]);
        assert_eq!(p.class_cap, vec![3, 2]);
    }

    #[test]
    fn bad_schools_are_refused_with_a_reason() {
        type Change = Box<dyn Fn(&mut School)>;
        let cases: Vec<(&str, Change)> = vec![
            ("days", Box::new(|s| s.days = 0)),
            ("days", Box::new(|s| s.days = 8)),
            ("periods", Box::new(|s| s.periods = 13)),
            ("no class", Box::new(|s| s.lessons[0].class = 2)),
            ("no teacher", Box::new(|s| s.lessons[0].teacher = 9)),
            ("no room", Box::new(|s| s.lessons[0].room = Some(1))),
            ("per_week", Box::new(|s| s.lessons[0].per_week = 0)),
            ("max_per_day", Box::new(|s| s.lessons[0].max_per_day = 0)),
            (
                "max_per_day",
                Box::new(|s| s.teachers[0].max_per_day = Some(0)),
            ),
            (
                "outside the week",
                Box::new(|s| s.teachers[0].unavailable[0].day = 2),
            ),
            (
                "outside the week",
                Box::new(|s| s.classes[0].unavailable.push(Slot { day: 0, period: 3 })),
            ),
            ("name", Box::new(|s| s.teachers[1].name = " ".into())),
            ("name", Box::new(|s| s.name = "x".repeat(81))),
            (
                "teachers 0 and 1 have the same name",
                Box::new(|s| {
                    s.teachers[1].name = format!(" {} ", s.teachers[0].name.to_uppercase())
                }),
            ),
            (
                "classes 0 and 1 have the same name",
                Box::new(|s| s.classes[1].name = s.classes[0].name.clone()),
            ),
            (
                "rooms 0 and 1 have the same name",
                Box::new(|s| s.rooms.push(s.rooms[0].clone())),
            ),
            (
                "4 hours pinned but 3 a week",
                Box::new(|s| {
                    s.lessons[0].pinned = (0..4)
                        .map(|i| Slot {
                            day: i / 3,
                            period: i % 3,
                        })
                        .collect()
                }),
            ),
            (
                "lesson 1: pinned slot day 0 period 3 is outside the week",
                Box::new(|s| s.lessons[1].pinned = vec![Slot { day: 0, period: 3 }]),
            ),
            (
                "lesson 1: pinned slot day 2 period 0 is outside the week",
                Box::new(|s| s.lessons[1].pinned = vec![Slot { day: 2, period: 0 }]),
            ),
            (
                "lesson 0: slot day 1 period 1 is pinned twice",
                Box::new(|s| s.lessons[0].pinned = vec![Slot { day: 1, period: 1 }; 2]),
            ),
            ("at least one lesson", Box::new(|s| s.lessons.clear())),
            (
                "hours",
                Box::new(|s| {
                    s.lessons = vec![s.lessons[0].clone(); 17];
                    for l in &mut s.lessons {
                        l.per_week = 250;
                    }
                }),
            ),
        ];
        for (reason, change) in cases {
            let mut school = tiny();
            change(&mut school);
            let err = Problem::new(&school).unwrap_err();
            assert!(err.contains(reason), "{reason}: {err}");
        }
    }
}
