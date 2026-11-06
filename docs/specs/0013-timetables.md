# 0013 — School timetables

Status: implemented (data entry, printing and real-school measurements are next)

## Problem

Every Bulgarian school builds a weekly timetable (седмично разписание) each term: which
class has which subject with which teacher, in which room, at which period. A deputy
director spends days on it, by hand or with paid single-computer software, and does it
again when a teacher leaves. The rules are simple to say and hard to satisfy together:
no teacher or class or room in two places at once, no gaps in a class's day, few gaps for
teachers, not three hours of one subject on one day.

Timetabling is a search problem. Many independent searches from different starting points
find better timetables than one search in the same time, so the room's devices help.
And a timetable is cheap to **check**: the coordinator re-scores every timetable a device
returns, exactly, so a device cannot claim a better timetable than it found.

## The school

`templates/tt` and `crates/timetable`. A school, as JSON (indices refer to the lists):

| Field | Meaning |
|---|---|
| `name` | e.g. „ПМГ Габрово, I срок“ |
| `days`, `periods` | days a week (1–7) and periods a day (1–12) |
| `teachers` | `{name, unavailable: [{day, period}], max_per_day?}` |
| `classes` | `{name, unavailable: [{day, period}]}` (паралелки) |
| `rooms` | `{name}`: rooms that must not be double-booked (labs, gym, a class's own room) |
| `lessons` | `{class, teacher, subject, per_week, room?, max_per_day}`: e.g. 10А, Петрова, Математика, 4 a week, at most 1 a day |

At most 200 teachers, 100 classes, 100 rooms, 2 000 lessons and 4 000 lesson hours a week.

## Cost

A timetable puts every lesson hour at a slot (day × period). Its cost is
`hard × 1 000 000 + soft`; lower is better, and a timetable with `hard = 0` is valid.

| Rule | Kind | Weight |
|---|---|---|
| a teacher, class or room has two lessons at one slot | hard | per extra lesson |
| a lesson at a slot its teacher or class is unavailable | hard | per lesson |
| a pinned hour away from its pin (spec 0015) | hard | per hour |
| a gap in a class's day (students wait between lessons) | soft | 100 per empty period |
| a class's day starts late (morning shift) | soft | 30 per empty period before its first lesson |
| a subject more often in a day than its `max_per_day` | soft | 50 per extra hour |
| a teacher's day longer than their `max_per_day` | soft | 20 per extra hour |
| a gap in a teacher's day | soft | 5 per empty period |
| a class's day longer than an even spread allows (`⌈hours ÷ days⌉ + 1`) | soft | 20 per extra hour |

All arithmetic is integer, so every device computes the same cost for the same timetable.

## Search

Each unit is one independent search: a greedy start, then **late acceptance hill
climbing** (Burke and Bykov), which accepts a change if it is no worse than the current
timetable or than the one `L` steps ago. Moves: one lesson hour to another slot, or two
lesson hours of one class swap slots. Costs are updated incrementally per (class, day),
(teacher, day) and slot. The randomness is a counter-based generator seeded from the job's
seed and the unit's index, so a unit is a pure function of its inputs: two honest devices
return the same timetable, bit for bit. Units differ in seed and in `L` (10, 20, 50, 100),
a portfolio of search styles; longer histories need more iterations than a unit has
(measured, `docs/measurements.md`).

## Jobs

- `template: "tt"`, params `{school, searches, iterations, seed}`: 1–500 searches of
  10 000 – 20 000 000 iterations; the work per unit is bounded so a phone finishes in
  seconds to a minute.
- Unit output: `{slots, cost}`, `slots` the base64 of one little-endian `u16` per lesson
  hour in lesson order. **The coordinator checks it**: the right number of slots, each in
  range, and the cost it recomputes equal to the cost claimed. A device cannot report a
  better timetable than the one it sends.
- Agreement is equality (the search is deterministic). On the public network units need
  two agreeing results as everything else; in a school's own network one is enough when
  its devices are trusted, because no device can fake a better result, only waste its
  unit.
- The job's result: the best timetable (lowest cost, earliest unit on ties), its cost
  broken down by rule, and every unit's cost (the spread shows what more searches buy).

## Pinning

A template's hash pins the kernel workers run. This kernel is Rust compiled to
WebAssembly, not WGSL, so the hash pins its behaviour instead: the hash of
`templates/tt/vectors.json`, the fixed inputs and the exact outputs the solver must give
for them. A change in the solver's results changes the vectors (a test fails until they
are updated) and so the hash; a browser with an older build refuses the units.

## Console

- A job kind „Разписание“: upload the school as JSON, or start from an example school
  (a generated Bulgarian high school of a chosen size).
- The job's page shows the best timetable per class and per teacher, its cost by rule,
  and the costs of all searches; it downloads the timetable as JSON and CSV.

Data entry for a deputy director (tables, import from a spreadsheet) and printing come
next, in their own spec.

## Acceptance criteria

1. The incremental cost after any sequence of moves equals the cost computed from scratch
   (property test).
2. A feasible example school gets a valid timetable (`hard = 0`) from one unit; more
   searches give a best cost at least as good (measured, `docs/measurements.md`).
3. A unit gives the same output natively and in WebAssembly (vectors in both, and a
   browser test in every engine).
4. A forged output (wrong cost claimed, slot out of range, wrong length) is refused and
   counts against the device; a lazy device that returns a valid but worse timetable is
   caught by disagreement on the public network.
5. A job runs end to end on simulated devices and in the browser, and the console shows
   the timetable.

## Verified by

`cargo nextest run` (unit, property and end-to-end tests), the vectors in Rust and in the
browser, Playwright in every engine, and measurements of quality against the number of
searches on generated schools of several sizes.
