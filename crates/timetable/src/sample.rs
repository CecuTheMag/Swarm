//! Example schools: a Bulgarian high school of a chosen size, generated from a seed.
//! For tests, measurements and trying the app without typing in a real school.

use crate::{
    model::{Class, Lesson, Room, School, Slot, Teacher},
    rng::Rng,
};

/// The weekly plan of every class: subject, hours a week, most hours a day, and the
/// kind of room it needs. About 31 hours a week, as in Bulgarian upper secondary.
const PLAN: [(&str, u8, u8, Option<RoomKind>); 15] = [
    ("Български език и литература", 4, 2, None),
    ("Математика", 4, 2, None),
    ("Английски език", 3, 1, None),
    ("Немски език", 2, 1, None),
    ("История и цивилизации", 2, 1, None),
    ("География и икономика", 2, 1, None),
    ("Биология и здравно образование", 2, 1, Some(RoomKind::Lab)),
    (
        "Химия и опазване на околната среда",
        2,
        1,
        Some(RoomKind::Lab),
    ),
    ("Физика и астрономия", 2, 1, Some(RoomKind::Lab)),
    ("Философия", 1, 1, None),
    ("Информационни технологии", 2, 1, Some(RoomKind::Computers)),
    ("Изобразително изкуство", 1, 1, None),
    ("Музика", 1, 1, None),
    ("Физическо възпитание и спорт", 2, 1, Some(RoomKind::Gym)),
    ("Час на класа", 1, 1, None),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoomKind {
    Lab,
    Computers,
    Gym,
}

const FIRST_NAMES: [&str; 12] = [
    "Мария",
    "Иван",
    "Елена",
    "Георги",
    "Десислава",
    "Николай",
    "Петя",
    "Димитър",
    "Радослава",
    "Стефан",
    "Цветелина",
    "Христо",
];
const LAST_NAMES: [&str; 12] = [
    "Петрова",
    "Иванов",
    "Георгиева",
    "Димитров",
    "Николова",
    "Стоянов",
    "Христова",
    "Тодоров",
    "Василева",
    "Атанасов",
    "Колева",
    "Маринов",
];

/// Most hours a week a teacher is given, as a full-time Bulgarian teacher's norm allows.
const TEACHER_LOAD: u32 = 21;
const DAYS: u8 = 5;
const PERIODS: u8 = 7;

/// A high school with `classes` classes (grades 8 to 12, letters А, Б, В…), five days of
/// seven periods, subject teachers with full loads, labs, computer rooms and gyms in
/// proportion, and a few part-time teachers away on some days.
pub fn school(seed: u64, classes: usize) -> School {
    let classes = classes.clamp(1, 100);
    let mut rng = Rng::new(seed);
    const LETTERS: [&str; 20] = [
        "А", "Б", "В", "Г", "Д", "Е", "Ж", "З", "И", "К", "Л", "М", "Н", "О", "П", "Р", "С", "Т",
        "У", "Ф",
    ];
    let class_list: Vec<Class> = (0..classes)
        .map(|i| Class {
            name: format!("{}{}", 8 + i % 5, LETTERS[i / 5]),
            unavailable: vec![],
        })
        .collect();

    // Rooms in proportion to the hours that need them, with slack for a feasible week.
    let week = u32::from(DAYS) * u32::from(PERIODS);
    let mut rooms = Vec::new();
    let mut room_ids = Vec::new();
    for (kind, name) in [
        (RoomKind::Lab, "Лаборатория"),
        (RoomKind::Computers, "Компютърен кабинет"),
        (RoomKind::Gym, "Физкултурен салон"),
    ] {
        let hours: u32 = PLAN
            .iter()
            .filter(|p| p.3 == Some(kind))
            .map(|p| u32::from(p.1))
            .sum::<u32>()
            * classes as u32;
        let count = hours.div_ceil(week * 3 / 4).max(1);
        let ids: Vec<u16> = (0..count)
            .map(|i| {
                rooms.push(Room {
                    name: format!("{name} {}", i + 1),
                });
                (rooms.len() - 1) as u16
            })
            .collect();
        room_ids.push((kind, ids));
    }

    // Teachers per subject, each filled up to a full load before the next is hired.
    let mut teachers: Vec<Teacher> = Vec::new();
    let mut lessons = Vec::new();
    for (subject, per_week, max_per_day, kind) in PLAN {
        let mut current: Option<(u16, u32)> = None;
        for c in 0..classes {
            let teacher = match current {
                Some((t, load)) if load + u32::from(per_week) <= TEACHER_LOAD => t,
                _ => {
                    let n = teachers.len();
                    teachers.push(Teacher {
                        name: format!(
                            "{} {}",
                            FIRST_NAMES[(n * 5 + 3) % FIRST_NAMES.len()],
                            LAST_NAMES[n % LAST_NAMES.len()]
                        ),
                        unavailable: vec![],
                        max_per_day: Some(6),
                    });
                    current = Some((n as u16, 0));
                    n as u16
                }
            };
            let load = current.map_or(0, |(_, l)| l) + u32::from(per_week);
            current = Some((teacher, load));
            let room = kind.map(|k| {
                let ids = &room_ids
                    .iter()
                    .find(|(rk, _)| *rk == k)
                    .expect("every kind")
                    .1;
                ids[c % ids.len()]
            });
            lessons.push(Lesson {
                class: c as u16,
                teacher,
                subject: subject.to_owned(),
                per_week,
                room,
                max_per_day,
            });
        }
    }

    // One teacher in eight works part time: away on one day of the week.
    for t in &mut teachers {
        if rng.below(8) == 0 {
            let day = rng.below(usize::from(DAYS)) as u8;
            t.unavailable = (0..PERIODS).map(|period| Slot { day, period }).collect();
        }
    }

    School {
        name: format!("Примерна гимназия ({classes} паралелки)"),
        days: DAYS,
        periods: PERIODS,
        teachers,
        classes: class_list,
        rooms,
        lessons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::problem::Problem;

    #[test]
    fn examples_are_valid_schools_of_the_size_asked() {
        for classes in [1, 5, 20, 40, 100] {
            let s = school(7, classes);
            let p = Problem::new(&s).unwrap();
            assert_eq!(p.classes, classes);
            // 31 hours a week per class, within the 35 periods of the week.
            assert!(p.class_hours.iter().all(|h| h.len() == 31));
            assert!(s.teachers.iter().all(|t| {
                let load: u32 = s
                    .lessons
                    .iter()
                    .filter(|l| s.teachers[usize::from(l.teacher)].name == t.name)
                    .map(|l| u32::from(l.per_week))
                    .sum();
                load > 0
            }));
        }
        assert_eq!(school(7, 20), school(7, 20));
        assert_ne!(school(7, 20), school(8, 20));
    }

    #[test]
    fn class_names_follow_grades_and_letters() {
        let s = school(1, 7);
        let names: Vec<&str> = s.classes.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["8А", "9А", "10А", "11А", "12А", "8Б", "9Б"]);
    }
}
