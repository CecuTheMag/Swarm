# 0015 — Pinned lessons and a timetable edited by hand

Status: implemented

## Problem

A search (spec 0013) treats every lesson hour alike. A school does not: PE for 8А is on
Monday's first period because the gym is free then, religion is on Thursday's last
period because the priest comes then, and the deputy director decided it in September.
Those hours must stay where they are put, and the search must fill the rest around them.

A found timetable is also never the last word. The deputy director looks at it, sees that
Петрова would rather have her free period on Friday, and moves two hours by hand. They
need to see at once what a move breaks (a clash, a teacher away, a gap), undo it, keep
the edited timetable, print it, and let the devices search again around what they fixed.

## Pinned lessons

- A lesson may pin some of its weekly hours: `pinned: [{day, period}]`, at most
  `per_week` slots, each in the week and each once. The first `pinned.len()` hours of the
  lesson are fixed at those slots, in order; hours of one lesson are interchangeable.
- The solver places pinned hours first and never moves them. A school without pins
  searches exactly as before: the same timetable for the same unit, bit for bit.
- A new hard rule, **an hour away from its pin**, so the cost is defined for any
  timetable: a device that moves a pinned hour gets a hard violation for it and cannot
  win, and the coordinator's checks are unchanged (it recomputes the cost).
- `Problem::new` refuses more pins than hours, a pin outside the week and a slot pinned
  twice in one lesson. The editor's checks (spec 0014) add two **problems**, which block
  a search: a pin at a slot its teacher or class is unavailable, and two pinned hours of
  one teacher, class or room at one slot.
- In the school editor, each lesson of the plan has a „Закрепи“ button that opens the
  week's grid: a click pins or unpins a slot, up to the lesson's hours a week. The CSV
  import and export have an optional column Закрепени, e.g. `Пн1 Ср3`.

## Editing a found timetable

On a done timetable job's page (`/app/n/{id}/jobs/{job}/timetable`), an admin turns on
„Редактирай“:

- A click (or Enter) on an hour selects it. Every cell of the class or teacher shown then
  says what moving the hour there would change: the new cost, and whether it breaks a
  hard rule. A click on a cell moves the hour there, or trades places with the one hour
  of that class or teacher already there. Hours pinned in the school cannot be moved.
- The cost by rule follows every move, computed by the same Rust code the coordinator
  runs, compiled to WebAssembly. Hard conflicts are listed by name: who, when, with what.
- A moved hour is pinned (📌) by default, so the next search keeps it; a click unpins it.
  „Назад“ undoes the last change, „Върни намереното“ restores the found timetable.
- **Saving.** `GET|PUT|DELETE /api/networks/{id}/jobs/{job}/timetable` reads, saves and
  drops the edited timetable of a done timetable job: `{slots, pinned, version}`, the
  slots as the job's result has them and the pinned hours by index. The coordinator
  refuses a timetable of the wrong length or with a slot outside the week, pinned hours
  out of range or given twice, and a save over a newer version (409); it recomputes the
  cost. Members read it, admins change it.
- Printing, the CSV and the JSON use the timetable shown: the edited one, once there is
  one.
- **Search again.** „Търси отново“ starts a new timetable job from the job's school with
  every pinned hour of the edit added to its lesson's pins, the same number of searches
  and iterations, and a new seed.

## Acceptance criteria

1. Every pinned hour is at its slot in every timetable the solver returns, and the cost
   kept move by move equals the cost from scratch with pins (property tests).
2. Schools without pins get the same timetables as before (the vectors of spec 0013
   unchanged); a new vector with pins pins the template's new behaviour.
3. Each new refusal and each new problem fires for a school built to fail it, and none
   for the example schools.
4. The cost the page shows equals the coordinator's for the same timetable (WebAssembly
   and native agree on the vectors and on edited timetables).
5. An edit saved and read back is the same; a save over a newer version, a viewer's save,
   another network's read, a job that is not a done timetable job, and malformed
   timetables are refused.
6. In the browser, in every engine: an admin moves an hour and sees the cost change,
   moves one onto a busy teacher and sees the clash named, undoes it, saves, reloads to
   find the edit, prints it, and searches again: the new timetable has the pinned hour
   where it was put.

## Verified by

`cargo nextest run` (solver, cost and refusals with pins; the edit API), Vitest (pins in
the editor's operations, checks and CSV; the WebAssembly cost), Playwright in every engine
(criterion 6).
