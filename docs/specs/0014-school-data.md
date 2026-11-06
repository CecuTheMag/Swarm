# 0014 — A school's data, entered by its deputy director

Status: implemented (pinning lessons and editing a found timetable are next)

## Problem

Spec 0013 searches for a timetable of any school given as JSON. Nobody at a school writes
JSON. A deputy director has the plan of the year in a spreadsheet (класове, предмети,
учители, часове седмично) and knows the rest by heart: which teacher is away on Wednesdays,
that the gym is shared, that 8А has its first period free on Mondays. The console must take
that in, keep it between visits, say what is wrong with it before a search starts, and
print the result for the staff room.

## The school, kept per network

- A network keeps any number of schools (a term, a draft, next year's plan), each a
  `School` of spec 0013 with a name; members see them, admins change them.
- `GET /api/networks/{id}/schools` lists them (id, name, size, updated, by whom);
  `POST` creates one, `GET|PUT|DELETE /api/networks/{id}/schools/{school}` reads, saves
  and deletes one. A save carries the version it was edited from, and a save over a newer
  version is refused (409), so two admins never overwrite each other unknowingly.
- The coordinator checks a school before saving it, as it does before a search
  (`Problem::new`), and also that no two teachers, classes or rooms share a name: a
  printed timetable says „Иванова“, and must mean one person. At most 50 schools a
  network, each at most 2 MiB.

## The editor (`/app/n/{id}/school/{school}`)

One page, five parts, all saved together:

| Part | What is entered |
|---|---|
| Седмица | days a week (5 or 6), periods a day |
| Учители | name, most hours a day, hours they cannot teach (a grid of the week: click to mark) |
| Паралелки | name, hours they have no lessons (the same grid) |
| Стаи | rooms that must not be double-booked: labs, the gym, computer rooms |
| Учебен план | per class: subject, teacher, hours a week, most a day, room if one is needed |

The plan is a table: one row per lesson, with a class filter; adding a row picks the
class, the teacher and the room from the lists, or types a new name, which is added to
its list. Renaming a teacher renames them everywhere; removing one who still teaches is
refused, with the lessons named.

**Import.** A CSV (or the CSV a spreadsheet saves, `;` or `,`, UTF-8 with or without BOM)
with the columns Паралелка, Предмет, Учител, Часове седмично, and optionally Най-много на
ден and Стая. Classes, teachers and rooms are created from the names. Every row that
cannot be read is reported with its line number and why; nothing is imported until every
row reads. A JSON school (spec 0013) imports as it is.

**Checks, live.** Before a search can start, the editor lists, in words and naming the
class, teacher or room, what makes the school impossible (**problems**, which block the
search): a class with more hours than its week has periods left to it, a teacher with
more hours than the hours they are available, a room needed more hours than the week has.
It also lists the soft rules that cannot all be met (**warnings**, which do not block):
a subject more often a week than its daily limit times the days, a teacher with more
hours than their daily limit times the days. These are counts only: a school can pass
them and still have no valid timetable, which the search then reports as hard conflicts.

**Searching.** „Търси разписание“ starts a timetable job (spec 0013) from the saved
school. The job keeps its own copy: later edits, or deleting the school, leave it as it
was searched.

## Printing

The timetable page prints the class or teacher shown, every class, or every teacher,
each on its own A4 landscape page, without the console around it, with the school's name
and the date it was printed. The browser's own print dialog saves it as PDF.

## Acceptance criteria

1. A school saved and read back is the same school; a save over a newer version is
   refused; a viewer cannot save; another network's members cannot read it.
2. Duplicate names and every rule of `Problem::new` are refused on save, with a message
   naming what is wrong.
3. The CSV import reads a spreadsheet's export of a real school's plan (`;`, BOM, quoted
   fields, blank lines, Cyrillic), and reports bad rows by line; a JSON school imports
   unchanged (property test: export then import is the identity).
4. Each impossibility check fires for a school built to fail it, and none fires for the
   example schools.
5. In the browser, a deputy director imports a CSV, marks a teacher away for a day,
   saves, reloads to find it as left, starts a search, and prints a class's timetable;
   in every engine.

## Verified by

`cargo nextest run` (storage, permissions, versions, validation), Vitest (CSV parsing,
checks, the editor's operations on the school), Playwright in every engine (the flow in
criterion 5, and print media emulated for the printed page).

Pinning a lesson to a slot, editing a found timetable, and searching again from the best
ones so far come next, in their own spec.
