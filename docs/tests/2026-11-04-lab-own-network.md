# Lab: a client's own network, on every screen

2026-11-04, school lab ([lab-method.md](lab-method.md)).

**What:** the product as a school would use it (spec 0010).

1. A test account (`lab@swarm.test`) was created on the server and signed in.
2. It created the network „Компютърен кабинет“ and got its join code.
3. Every PC opened `/join/<code>` in a visible, maximized Edge on the student's screen
   and pressed „Включи се“; the console showed the network on the laptop.
4. The console's account submitted a π job to the network: 600 units of 2²⁸ samples.

**Result:**

| | |
|---|---|
| Devices in the network | 13 |
| Samples | 161 061 273 600 |
| Time | 60 s |
| π | 3.1415943776071074 |
| Progress seen | 37, 110, 190, 268, 347, 424, 503, 578, 600 units at 6–9 s intervals |

The deploy that added spec 0011 (neural network sweeps) went out minutes before, and
the network's jobs only went to its own devices.

**Found:**

- One PC showed „Страницата е скрита“ and did not compute: its window counted as hidden
  to the browser. Opening the page with `?background` fixed it. A lab should either
  keep the worker window in front or use `?background`.
- `coordinator admin-create` only makes platform admins, so the test account is one.
  Accounts for clients should come from invites (spec 0010); a way to delete an
  account is missing.
