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
