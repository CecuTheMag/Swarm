# Design system

How the Рояк web app looks and moves. Everything lives in `web/src/app.css` (tokens and
components) and `web/src/lib/ui/` (logo, icons, motion); pages only lay these out.

## Principles

- **For people without a technical background.** One obvious action per screen, plain
  Bulgarian, the same word for the same thing everywhere („части“ for units of work,
  „устройство“ for any device, „Включи се“ to join, „Спри“ to stop).
- **Quiet and confident.** Neutral surfaces, one brand colour (honey amber), large type,
  generous space. No decoration that does not explain something.
- **Glass with a purpose.** Frosted panels float over a soft ambient light and a faint
  honeycomb; they fall back to solid surfaces where blur is missing or the user asks
  for less transparency.
- **Motion that settles.** Short, soft easing; nothing bounces. All of it stops for
  `prefers-reduced-motion`.

## Refinement brief (2026-11-23)

The same product, finished more carefully. Nothing here replaces the look; it removes
what does not explain something and makes what is left exact.

**Stays.** Honey amber as the only brand colour, the neutral surfaces, frosted glass, the
ambient light and the faint honeycomb, Inter Variable, light and dark following the
system, `.theme-dark` for the projector, every token name, every route, all copy and
terms („части“, „устройство“, „Включи се“, „Спри“), the protection defaults of spec 0007.

**Refined.**

- *Depth: three levels, one rule each.* Page (background, ambient light). Card: a solid
  surface with a hairline border and a small shadow, for content that sits on the page.
  Glass: blur, highlight and the larger shadow, only for layers that float over moving
  content (the sticky header, the phone menu, the hero's live field, the worker's
  console). Cards no longer blur: it explained nothing and cost the GPU on the worker
  page, where it matters most.
- *Type.* Tracking no tighter than −0.04 em; heading sizes step clearly (display, 2xl,
  xl, lg); every counter uses tabular numerals so live numbers do not jitter; numbers
  are written the Bulgarian way (`5,6`, `1 000`).
- *Colour.* Amber and neutrals carry the interface; red, green and blue appear only for
  status. Device kinds on the projector and the home field are told apart by amber
  weight (solid, light, neutral), not by extra hues. No gradient text.
- *Home page.* The section kickers above headings go: the headings carry themselves.
  Fewer boxes: the three steps read as a sequence, the organisation features as a
  list; cards stay for the three kinds of work. Icon tiles become small inline icons.
  Cards that are not links no longer lift on hover.
- *The honeycomb is live.* The hero's field shows the real network from the live
  dashboard stream: a cell appears when a device joins, lights when that device
  returns finished work, dims when it has been idle for ten seconds, fades out when it
  leaves and turns red if it is caught cheating. Empty cells are room for more. No
  invented activity: with no devices the field is empty and says so.
- *Browser surfaces.* Focus ring, caret, selection and scrollbars take the palette; the
  focus ring meets 3:1 against the page in both themes.

**Motion rules.**

1. Every motion explains feedback, state or continuity. Nothing loops unless it shows
   that work is happening right now.
2. Curves settle: `--ease` (strong ease-out) for arrivals and feedback, `--ease-in-out`
   for things moving on screen. No overshoot anywhere: no springs that bounce, no
   scale past 1.
3. Durations: press 120–160 ms, state changes `--fast`/`--base`, entrances `--slow`.
   Exits are faster than entrances. Reveal on scroll moves 12 px, not 24.
4. Only `transform` and `opacity` animate continuously (they run on the compositor).
   Width, shadows and colours may transition once on a state change, never in a loop.
5. The live field is the one authored moment, on the home page only. It draws to one
   canvas, at most 30 frames per second, only while something is changing, and stops
   when the tab is hidden or the field is off screen. Its frame cost is measured.
6. The worker page stays light while a device computes: no canvas, no blur behind
   moving content, and its two loops (the turning ring, the live dot) run on the
   compositor.
7. `prefers-reduced-motion`: the same information, static. The field draws each state
   directly (active cells full, idle cells dim) without flashes or fades; the ring
   shows its arc without turning; reveals are off.

**Per page.**

| Page | Changes |
|---|---|
| Home (`/`) | Live field in the hero; solid accent instead of gradient text; kickers removed; steps as a sequence, features as a list; inline icons; Bulgarian number format |
| Worker (`/worker`) | Cards solid; ring turns on the compositor; consent text one step larger; counters tabular |
| Login, invite | Shared focus, field and card refinements only |
| Projector (`/render/…`, `…/live`) | Swarm map without bounce or glyph icons, amber-weight device kinds, tabular counters; still fits 1920×1080 and 1600×900 |
| Console (`/app/…`) | Inherits the token, card and focus refinements |

## Tokens

| Group | Tokens | Notes |
|---|---|---|
| Brand | `--brand`, `--brand-strong`, `--brand-ink`, `--brand-soft`, `--on-brand` | Fills carry `--on-brand` text; amber text uses `--brand-ink` (5.8:1 on light) |
| Neutrals | `--bg`, `--bg-elevated`, `--text`, `--muted`, `--faint`, `--border`, `--border-strong` | `--faint` is the smallest allowed contrast for text (≥ 4.5:1) |
| Glass | `--glass`, `--glass-strong`, `--glass-border`, `--glass-highlight`, `--glass-blur` | |
| Semantic | `--ok`, `--bad`, `--info` and their `-soft` fills, `--on-bad` | |
| Type | `--font` (Inter Variable, self-hosted, Cyrillic), `--text-xs` … `--text-display` | Body 17 px; headings tight (−0.025 em) |
| Space | `--s-1` … `--s-10` (4 px steps), `--gutter`, `--page` | |
| Shape | `--radius-sm` 10, `--radius` 16, `--radius-lg` 24, `--radius-xl` 32, `--pill` | |
| Depth | `--shadow-sm`, `--shadow`, `--shadow-lg` | |
| Motion | `--ease`, `--ease-in-out`; `--fast` 160, `--base` 280, `--slow` 560, `--slower` 900 ms | |

Light and dark follow the system; `.theme-dark` forces dark (the projector page).

## Components

| Class | Variants | States and rules |
|---|---|---|
| `button`, `.button` | default (amber), `.secondary` (glass), `.ghost`, `.danger`; `.sm`, `.lg` | hover, press (scale 0.97), focus ring, disabled; ≥ 44 px tall on touch screens |
| `.segmented` | — | selected item (`aria-pressed`/`aria-checked`) is a dark pill, ≥ 3:1 against the track |
| `.glass`, `.card` | — | frosted panel; `.card` adds padding |
| `input`, `select`, `label` | — | 16 px text (no zoom on iPhone), amber focus halo; labels wrap their field |
| `.notice` | info, `.warn`, `.bad`, `.ok` | icon + one sentence: what happened and what happens next |
| `.badge` | `.live`, `.ok`, `.bad` | with `.dot` (`.pulse` for live) |
| `.stat` | — | `.value` + `.label` |
| `.progress` | — | eases to its new width |
| `.eyebrow`, `.page-head` | — | section label; page title and lead |

## Motion

- Sections fade up on first view: `use:reveal={delay}` (`#lib/ui/reveal.ts`); hidden
  only when JavaScript runs (`html.js`), so nothing is lost without it.
- Pages cross-fade (View Transitions) where supported.
- The worker's ring turns while the device takes part, faster while it computes; the
  start button breathes.

## Accessibility (checked 2026-11-03)

- Contrast ≥ 4.5:1 for text in both themes, including on glass and the projector's
  alarm; large display text ≥ 3:1.
- Visible focus ring on every control, keeping the control's shape; „Към съдържанието“
  skip link first in tab order.
- Landmarks: header navigation, main content, footer. Decorative SVG is hidden from
  screen readers; status changes on the worker page are announced politely.
- No page scrolls sideways from 320 px up; the projector page fits 1920×1080 and
  1600×900 without scrolling.
