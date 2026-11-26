// Builds the defense deck: ../Рояк.pptx (with Morph transitions and entrance
// animations) and ../сценарий.md (the script). `npm run build` in this folder.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pptxgen from "pptxgenjs";
import { buildAssets, iconData, SIZES } from "./assets.mjs";
import { animate } from "./animate.mjs";
import { SLIDES as SCRIPT, INTRO, OUTRO } from "./script.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR = path.resolve(here, "..");
const DECK = path.join(OUT_DIR, "Рояк.pptx");

// ---------------------------------------------------------------- design tokens
const C = {
  bg: "0A0A0C",
  card: "141417",
  card2: "1C1C21",
  line: "2B2B31",
  text: "F5F5F7",
  muted: "A8A8AE",
  faint: "74747C",
  amber: "FFB53C",
  amber2: "F5A524",
  amberDeep: "8A5300",
  blue: "4DA3FF",
  red: "FF5B54",
  green: "3DDC84",
  idle: "2E2E34",
};
const F = { h: "Helvetica Neue", m: "Helvetica Neue Medium", l: "Helvetica Neue Light" };
const THEME = {
  name: "Рояк",
  headFontFace: F.h,
  bodyFontFace: F.h,
  colors: {
    dk1: C.bg, lt1: "FFFFFF", dk2: C.card, lt2: C.text,
    accent1: C.amber, accent2: C.blue, accent3: C.green, accent4: C.red, accent5: C.muted, accent6: C.amber2,
    hlink: C.amber, folHlink: C.amber2,
  },
};

const A = await buildAssets();
const pres = new pptxgen();
pres.layout = "LAYOUT_WIDE"; // 13.333 × 7.5 in
pres.author = "St. Andonov";
pres.title = "Рояк — защита на проекта, НОИТ 2027";
pres.subject = "Доброволен суперкомпютър от браузъри";
pres.theme = { headFontFace: F.h, bodyFontFace: F.h };

// ---------------------------------------------------------------- layouts
const TITLE_PH = {
  placeholder: {
    options: {
      name: "title", type: "title", x: 0.75, y: 0.98, w: 11.8, h: 1.3,
      fontFace: F.h, fontSize: 38, bold: true, color: C.text, valign: "top", align: "left", margin: 0,
      lineSpacingMultiple: 0.92,
    },
    text: "",
  },
};
const FOOTER = [
  { image: { path: A.logo, x: 0.75, y: 6.93, w: 0.2, h: 0.2 } },
  { text: { text: "Рояк", options: { x: 1.02, y: 6.89, w: 2, h: 0.28, fontFace: F.m, fontSize: 11, color: C.faint, margin: 0, valign: "middle" } } },
];
for (const bg of ["topRight", "bottomLeft", "center", "blue", "red", "plain"]) {
  pres.defineSlideMaster({
    title: `Content-${bg}`,
    background: { path: A[`bg-${bg}`] },
    objects: [...FOOTER, TITLE_PH],
    slideNumber: { x: 11.9, y: 6.89, w: 0.68, h: 0.28, fontFace: F.m, fontSize: 11, color: C.faint, align: "right" },
  });
}
pres.defineSlideMaster({ title: "Hero", background: { path: A["bg-hero"] }, objects: [] });
pres.defineSlideMaster({
  title: "Statement",
  background: { path: A["bg-center"] },
  objects: [],
});
const L = (bg) => `Content-${bg}`;

// ---------------------------------------------------------------- slide context
const plans = [];
const notes = [];
let section = "";
function setSection(title) {
  section = title;
  pres.addSection({ title });
}

// `hidden` slides are skipped by the slideshow but still open by number (type it, Enter).
function newSlide(layout, { transition = { type: "morph", dur: 1400 }, hidden = false } = {}) {
  const s = pres.addSlide({ masterName: layout, sectionTitle: section });
  const plan = { transition, groups: [], hidden };
  plans.push(plan);
  const counters = {};
  const groupNames = {};
  const name = (g) => {
    if (g.startsWith("!!")) return g;
    counters[g] = (counters[g] ?? 0) + 1;
    const n = `${g}__${counters[g]}`;
    (groupNames[g] ??= []).push(n);
    return n;
  };
  const ctx = {
    s,
    title(text) {
      s.addText(text, { placeholder: "title" });
    },
    eyebrow(text, color = C.amber) {
      ctx.text("eyebrow", text, { x: 0.75, y: 0.6, w: 9, h: 0.3, size: 12.5, font: F.h, bold: true, color, charSpacing: 2.5 });
    },
    text(g, text, o = {}) {
      const { size = 16, font = F.h, color = C.text, ...rest } = o;
      text = typeof text === "string" ? nbsp(text) : text.map((r) => ({ ...r, text: nbsp(r.text) }));
      s.addText(text, {
        isTextBox: true, margin: 0, valign: "top", fontFace: font, fontSize: size, color,
        objectName: name(g), ...rest,
      });
    },
    shape(g, type, o) {
      s.addShape(type, { objectName: name(g), ...o });
    },
    image(g, o) {
      s.addImage({ objectName: name(g), ...o });
    },
    async icon(g, icon, o) {
      s.addImage({ objectName: name(g), data: await iconData(icon, o.color ?? C.amber), x: o.x, y: o.y, w: o.size, h: o.size, altText: "" });
    },
    chart(g, type, data, o) {
      s.addChart(type, data, { objectName: name(g), ...o });
    },
    hex(nameOrGroup, { cx, cy, r, fill = C.amber, t = 0, line = null, lineW = 1 }) {
      s.addShape(pres.ShapeType.hexagon, {
        objectName: name(nameOrGroup),
        x: cx - r, y: cy - r * 0.8660254, w: 2 * r, h: 1.7320508 * r,
        fill: { color: fill, transparency: t },
        line: line ? { color: line, width: lineW } : { type: "none" },
      });
    },
    // A group of entrance effects. Items name either a group prefix (`g`) or a
    // morph name (`name`); a group expands to all of its shapes, same timing.
    anim(trigger, items) {
      const expanded = [];
      for (const it of items) {
        if (it.name) expanded.push(it);
        else for (const n of groupNames[it.g] ?? fail(`no group ${it.g}`)) expanded.push({ ...it, name: n });
      }
      plan.groups.push({ trigger, items: expanded });
    },
    notes(i) {
      const part = SCRIPT[i];
      s.addNotes(`[${part.time}]\n${part.say}`);
      notes.push(part);
    },
  };
  return ctx;
}
// No line break inside "1 240", "70 s" or "75 %": a no-break space keeps them together.
function nbsp(t) {
  return t
    .replace(/(\d) (?=\d{3}\b)/g, "$1\u00A0")
    .replace(/(\d) (?=(s|ms|min|%|°C|MB\/s|млн\.|млрд\.)(?![\p{L}]))/gu, "$1\u00A0");
}
function fail(msg) {
  throw new Error(msg);
}

async function card(x, { g, ctx, y, w, h, icon, iconColor = C.amber, head, body, headSize = 20, bodySize = 15, num, fill = C.card, accent }) {
  ctx.shape(g, pres.ShapeType.roundRect, {
    x, y, w, h, rectRadius: 0.14, fill: { color: fill }, line: { color: accent ?? C.line, width: 1 },
  });
  let ty = y + 0.32;
  if (icon) {
    ctx.shape(g, pres.ShapeType.ellipse, { x: x + 0.3, y: y + 0.3, w: 0.62, h: 0.62, fill: { color: iconColor, transparency: 84 }, line: { type: "none" } });
    await ctx.icon(g, icon, { x: x + 0.43, y: y + 0.43, size: 0.36, color: iconColor });
    if (num) ctx.text(g, num, { x: x + w - 0.9, y: y + 0.36, w: 0.6, h: 0.3, size: 13, font: F.m, color: C.faint, align: "right" });
    ty = y + 1.15;
  }
  ctx.text(g, head, { x: x + 0.3, y: ty, w: w - 0.6, h: 0.45, size: headSize, bold: true });
  if (body) ctx.text(g, body, { x: x + 0.3, y: ty + headSize / 72 + 0.18, w: w - 0.6, h: h - (ty - y) - 0.5, size: bodySize, color: C.muted, lineSpacingMultiple: 1.05 });
}

// ---------------------------------------------------------------- the swarm of 19 cells
// Axial coordinates of a hexagonal patch: the centre, then rings 1 and 2.
const PATCH = (() => {
  const dirs = [[1, 0], [1, -1], [0, -1], [-1, 0], [-1, 1], [0, 1]];
  const cells = [[0, 0]];
  for (let ring = 1; ring <= 2; ring++) {
    let q = -ring, r = ring; // start at direction 4 × ring
    for (let side = 0; side < 6; side++) {
      for (let k = 0; k < ring; k++) {
        cells.push([q, r]);
        q += dirs[side][0];
        r += dirs[side][1];
      }
    }
  }
  return cells;
})();
const patchXY = (cx, cy, size) => PATCH.map(([q, r]) => [cx + size * 1.5 * q, cy + size * Math.sqrt(3) * (r + q / 2)]);
const CELL = (i) => `!!c${String(i).padStart(2, "0")}`;
// The live view's palette: amber = a GPU device, blue = a CPU device, dark = waiting.
const HERO_STYLE = [
  [C.amber, 0],
  [C.amber, 0], [C.amber2, 0], [C.amber, 15], [C.blue, 0], [C.amber, 0], [C.amber2, 10],
  [C.amber, 35], ["empty"], [C.amber, 55], [C.blue, 30], ["empty"], [C.amber, 20],
  [C.amber, 60], ["empty"], [C.blue, 0], [C.amber, 40], ["empty"], [C.amber, 25],
];
function swarm(ctx, positions, { r, style = HERO_STYLE, mono = null, monoT = null }) {
  positions.forEach(([cx, cy], i) => {
    const [fill, t] = mono ? [mono, typeof monoT === "function" ? monoT(i) : monoT ?? 0] : style[i];
    if (fill === "empty") ctx.hex(CELL(i), { cx, cy, r, fill: C.card2, t: 0, line: "3A3A41", lineW: 1 });
    else ctx.hex(CELL(i), { cx, cy, r, fill, t });
  });
}
const ringOrder = (delay0, step) => PATCH.map((_, i) => ({ name: CELL(i), fx: "pop", dur: 650, delay: delay0 + (i === 0 ? 0 : i <= 6 ? step * (1 + i * 0.35) : step * (3 + (i - 6) * 0.3)) }));

// ================================================================= SECTION: Откриване
setSection("Откриване");

// 1 — Title
{
  const x = newSlide("Hero", { transition: { type: "fade", dur: 900 } });
  x.image("glow", { path: A["glow-amber"], x: 6.25, y: 0.25, w: 7, h: 7, transparency: 30 });
  swarm(x, patchXY(9.75, 3.75, 0.74), { r: 0.69 });
  x.text("kicker", "НОИТ 2027 · РАЗПРЕДЕЛЕНИ ПРИЛОЖЕНИЯ", { x: 0.75, y: 2.05, w: 6.2, h: 0.32, size: 12.5, bold: true, color: C.amber, charSpacing: 2.5 });
  x.text("name", "Рояк", { x: 0.68, y: 2.42, w: 6.2, h: 1.75, size: 116, bold: true, lineSpacingMultiple: 0.9 });
  x.text("tag", "Суперкомпютър от браузърите\nв тази зала.", { x: 0.75, y: 4.3, w: 6.2, h: 1.1, size: 28, font: F.l, color: C.text, lineSpacingMultiple: 1.0 });
  x.text("url", "swarmrelay.duckdns.org", { x: 0.75, y: 5.75, w: 6.2, h: 0.35, size: 14, font: F.m, color: C.muted });
  x.anim("auto", [
    { g: "glow", fx: "fade", dur: 1600, delay: 0 },
    ...ringOrder(150, 140),
    { g: "kicker", fx: "float", dur: 800, delay: 500 },
    { g: "name", fx: "float", dur: 1000, delay: 650 },
    { g: "tag", fx: "float", dur: 900, delay: 900 },
    { g: "url", fx: "fade", dur: 900, delay: 1300 },
  ]);
  x.notes(0);
}

// 2 — Hook: the cells scatter into the devices of the room
{
  const x = newSlide("Statement", { transition: { type: "morph", dur: 1900 } });
  let seed = 11;
  const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
  const pts = [];
  while (pts.length < 19) {
    const p = [0.6 + rnd() * 12.1, 0.5 + rnd() * 6.5];
    const inText = p[0] > 0.9 && p[0] < 12.4 && p[1] > 2.05 && p[1] < 5.55;
    if (!inText && pts.every((q) => Math.hypot(q[0] - p[0], q[1] - p[1]) > 1.25)) pts.push(p);
  }
  pts.forEach(([cx, cy], i) => {
    const blue = [4, 10, 15].includes(i);
    x.hex(CELL(i), { cx, cy, r: 0.13 + ((i * 7) % 5) * 0.025, fill: blue ? C.blue : i % 3 ? C.amber : C.amber2, t: 0 });
  });
  x.text("l1", [
    { text: "Във всеки джоб в тази зала", options: { breakLine: true } },
    { text: "има графичен процесор.", options: { color: C.amber } },
  ], { x: 0.9, y: 2.45, w: 11.5, h: 1.8, size: 50, bold: true, align: "center", lineSpacingMultiple: 0.98 });
  x.text("l2", "Заедно тези устройства са суперкомпютър. Никой не ги е свързал.", { x: 0.9, y: 4.5, w: 11.5, h: 0.6, size: 24, font: F.l, color: C.muted, align: "center" });
  x.anim("auto", [
    { g: "l1", fx: "float", dur: 1000, delay: 200 },
    { g: "l2", fx: "float", dur: 1000, delay: 1500 },
  ]);
  x.notes(1);
}

// 3 — The problem: the cells go dark, a row of idle machines
{
  const x = newSlide(L("topRight"), { transition: { type: "morph", dur: 1600 } });
  x.eyebrow("ПРОБЛЕМЪТ");
  x.title("Мощта вече е в училището.\nСтои неизползвана.");
  PATCH.forEach((_, i) => x.hex(CELL(i), { cx: 0.95 + i * 0.633, cy: 6.22, r: 0.165, fill: C.idle, t: 0 }));
  const cols = [
    ["~75 %", "от седмицата нашият компютърен кабинет стои свободен. Оценка по разписанието му."],
    ["0", "инсталации позволява училищният компютър. BOINC и Folding@home искат права на администратор."],
    ["Дни", "съставя заместник-директорът седмичното разписание — на ръка или с програма на един компютър."],
  ];
  cols.forEach(([big, small], i) => {
    const cx = 0.75 + i * 4.1;
    x.text(`col${i}`, big, { x: cx, y: 2.75, w: 3.7, h: 1.25, size: 72, font: F.l, color: C.amber });
    x.text(`col${i}`, small, { x: cx, y: 4.05, w: 3.55, h: 1.4, size: 17, color: C.muted, lineSpacingMultiple: 1.08 });
  });
  x.anim("auto", [0, 1, 2].map((i) => ({ g: `col${i}`, fx: "float", dur: 900, delay: 400 + i * 260 })));
  x.notes(2);
}

// 4 — The idea
{
  const x = newSlide(L("bottomLeft"), { transition: { type: "morph", dur: 1600 } });
  x.eyebrow("ИДЕЯТА");
  x.title("Едно отваряне на страница.\nНищо не се инсталира.");
  swarm(x, patchXY(11.85, 1.3, 0.19), { r: 0.175 });
  const steps = [
    ["scan", "Включване", "QR код или връзка. Компютри в кабинета, лаптопи и телефони — всеки модерен браузър.", "01"],
    ["cpu", "Изчисляване", "На видеокартата с WebGPU. Без видеокарта — на процесора с WebAssembly. В отделна нишка.", "02"],
    ["check", "Проверка", "Всяка част се изчислява от две устройства. Контролни части и репутация хващат измамниците.", "03"],
  ];
  for (let i = 0; i < 3; i++) {
    const [icon, head, body, num] = steps[i];
    await card(0.75 + i * 4.1, { g: `step${i}`, ctx: x, y: 2.8, w: 3.7, h: 3.15, icon, head, body, num, headSize: 22, bodySize: 16 });
    if (i < 2) x.shape(`arrow${i}`, pres.ShapeType.chevron, { x: 4.5 + i * 4.1, y: 4.2, w: 0.2, h: 0.36, fill: { color: C.amber, transparency: 30 }, line: { type: "none" } });
  }
  x.anim("auto", [
    { g: "step0", fx: "float", dur: 800, delay: 400 },
    { g: "arrow0", fx: "left", dur: 500, delay: 800 },
    { g: "step1", fx: "float", dur: 800, delay: 900 },
    { g: "arrow1", fx: "left", dur: 500, delay: 1300 },
    { g: "step2", fx: "float", dur: 800, delay: 1400 },
  ]);
  x.notes(3);
}

// 5 — Join now: the cells ring the phone
{
  const x = newSlide(L("center"), { transition: { type: "morph", dur: 1700 } });
  const pcx = 3.35, pcy = 3.95;
  PATCH.forEach((_, i) => {
    const a = -Math.PI / 2 + (i / 19) * Math.PI * 2;
    x.hex(CELL(i), { cx: pcx + 2.62 * Math.cos(a), cy: pcy + 2.62 * Math.sin(a), r: 0.15, fill: [4, 10, 15].includes(i) ? C.blue : C.amber, t: (i * 37) % 60 });
  });
  x.image("glow", { path: A["glow-amber"], x: pcx - 2.6, y: pcy - 2.6, w: 5.2, h: 5.2, transparency: 55 });
  x.shape("phone", pres.ShapeType.roundRect, { x: 1.86, y: 1.66, w: 2.98, h: 4.58, rectRadius: 0.38, fill: { color: "050506" }, line: { color: "3A3A41", width: 1.5 } });
  x.shape("phone", pres.ShapeType.roundRect, { x: 1.98, y: 1.78, w: 2.74, h: 4.34, rectRadius: 0.3, fill: { color: "FAFAFB" }, line: { type: "none" } });
  x.image("phone", { path: A.phone, x: 1.98, y: 1.95, w: 2.74, h: 2.74 * (SIZES.phone[1] / SIZES.phone[0]), altText: "Страницата за доброволеца на телефон: бутон „Спри“ и „Помагате в момента“" });
  x.text("pill", [{ text: "●  ", options: { color: C.red } }, { text: "НА ЖИВО" }], { x: 6.55, y: 1.25, w: 2.2, h: 0.32, size: 12.5, bold: true, color: C.text, charSpacing: 2.5 });
  x.text("head", "Включете се сега.", { x: 6.5, y: 1.72, w: 6.3, h: 0.95, size: 46, bold: true });
  x.text("lead", "Сканирайте QR кода на екрана с телефона си.", { x: 6.55, y: 2.85, w: 6.0, h: 0.9, size: 24, font: F.l, color: C.text });
  const rows = [
    ["box", "Нищо не се инсталира"],
    ["gauge", "Работи на 75 % мощност"],
    ["stop", "„Спри“ спира всичко"],
  ];
  for (let i = 0; i < 3; i++) {
    const y = 4.15 + i * 0.68;
    x.shape(`row${i}`, pres.ShapeType.ellipse, { x: 6.55, y, w: 0.46, h: 0.46, fill: { color: C.amber, transparency: 84 }, line: { type: "none" } });
    await x.icon(`row${i}`, rows[i][0], { x: 6.65, y: y + 0.1, size: 0.26 });
    x.text(`row${i}`, rows[i][1], { x: 7.2, y: y + 0.07, w: 5, h: 0.4, size: 19, color: C.text });
  }
  x.anim("auto", [
    { g: "glow", fx: "fade", dur: 1200, delay: 0 },
    { g: "phone", fx: "zoom", dur: 900, delay: 100 },
    { g: "pill", fx: "fade", dur: 600, delay: 400 },
    { g: "head", fx: "float", dur: 900, delay: 500 },
    { g: "lead", fx: "float", dur: 900, delay: 750 },
    ...[0, 1, 2].map((i) => ({ g: `row${i}`, fx: "left", dur: 700, delay: 1100 + i * 200 })),
  ]);
  x.notes(4);
}

// ================================================================= SECTION: Демонстрация
setSection("Демонстрация");

// 6 — Demo 1: render and chaos (also the fallback if the network fails)
{
  const x = newSlide(L("red"), { transition: { type: "morph", dur: 1200 } });
  x.eyebrow("ДЕМОНСТРАЦИЯ 1");
  x.title("Картина, изчислена от залата");
  const iw = 7.45, ih = iw * (SIZES.liveRender[1] / SIZES.liveRender[0]);
  x.shape("shot", pres.ShapeType.roundRect, { x: 0.7, y: 2.2, w: iw + 0.1, h: ih + 0.1, rectRadius: 0.12, fill: { color: C.card }, line: { color: C.line, width: 1 } });
  x.image("shot", { path: A.liveRender, x: 0.75, y: 2.25, w: iw, h: ih, rounding: false, altText: "Изгледът на живо: картина с две стъклени сфери, изчислена плочка по плочка" });
  const events = [
    [C.amber, "Устройство изчезва", "Частите му се връщат в опашката и се изчисляват от друго устройство."],
    [C.red, "Измамник", "Хванат за 3,1–3,4 s и блокиран. Работата му се проверява отново."],
    [C.red, "Двама в сговор", "Хванати за около 70 s. Приетите им 49 части — преизчислени за 11 s."],
  ];
  events.forEach(([color, head, body], i) => {
    const y = 2.25 + i * 1.45;
    x.shape(`ev${i}`, pres.ShapeType.roundRect, { x: 8.55, y, w: 4.05, h: 1.3, rectRadius: 0.12, fill: { color: C.card }, line: { color: C.line, width: 1 } });
    x.shape(`ev${i}`, pres.ShapeType.ellipse, { x: 8.82, y: y + 0.3, w: 0.16, h: 0.16, fill: { color }, line: { type: "none" } });
    x.text(`ev${i}`, head, { x: 9.15, y: y + 0.2, w: 3.3, h: 0.36, size: 17, bold: true });
    x.text(`ev${i}`, body, { x: 9.15, y: y + 0.58, w: 3.3, h: 0.66, size: 14, color: C.muted, lineSpacingMultiple: 1.02 });
  });
  x.anim("auto", [{ g: "shot", fx: "zoom", dur: 1000, delay: 200 }]);
  for (let i = 0; i < 3; i++) x.anim("click", [{ g: `ev${i}`, fx: "right", dur: 700, delay: 0 }]);
  x.notes(5);
}

// 7 — Demo 2: a school's timetable
{
  const x = newSlide(L("bottomLeft"), { transition: { type: "morph", dur: 1200 } });
  x.eyebrow("ДЕМОНСТРАЦИЯ 2");
  x.title("Разписанието на едно училище");
  x.text("lead", "Заместник-директорът въвежда учебния план от електронна таблица. Всички устройства търсят разписанието едновременно, а координаторът преизчислява оценката на всяко предложено.", { x: 0.75, y: 2.2, w: 6.1, h: 1.5, size: 18, color: C.muted, lineSpacingMultiple: 1.08 });
  const stats = [
    ["1 240", "часа седмично в училище с 40 паралелки"],
    ["0,4 s", "за 2 милиона стъпки на търсене"],
    ["60 → 35", "цена с едно срещу две и повече търсения (по-малко е по-добре)"],
  ];
  stats.forEach(([big, small], i) => {
    const y = 3.85 + i * 0.9;
    x.text(`st${i}`, big, { x: 0.75, y, w: 2.55, h: 0.75, size: 38, font: F.l, color: C.amber, valign: "middle" });
    x.text(`st${i}`, small, { x: 3.4, y: y + 0.05, w: 3.5, h: 0.7, size: 15, color: C.text, valign: "middle" });
  });
  const tw = 4.85, th = tw * (SIZES.timetable[1] / SIZES.timetable[0]);
  x.shape("tt", pres.ShapeType.roundRect, { x: 7.7, y: 1.95, w: tw, h: th, rectRadius: 0.16, fill: { color: "FFFFFF" }, line: { type: "none" }, shadow: { type: "outer", color: "000000", opacity: 0.55, blur: 24, offset: 6, angle: 90 } });
  x.image("tt", { path: A.timetable, x: 7.78, y: 2.03, w: tw - 0.16, h: th - 0.16, altText: "Разписание на паралелка 8А: валидно разписание, оценка 45" });
  x.anim("auto", [
    { g: "tt", fx: "float", dur: 1000, delay: 200 },
    { g: "lead", fx: "fade", dur: 800, delay: 400 },
    ...[0, 1, 2].map((i) => ({ g: `st${i}`, fx: "left", dur: 700, delay: 700 + i * 220 })),
  ]);
  x.notes(6);
}

// ================================================================= SECTION: Как работи
setSection("Как работи");

// 8 — Architecture
{
  const x = newSlide(L("center"), { transition: { type: "morph", dur: 1200 } });
  x.eyebrow("КАК РАБОТИ");
  x.title("Архитектура");
  x.text("devH", "УСТРОЙСТВА", { x: 0.75, y: 2.2, w: 3.3, h: 0.3, size: 12, bold: true, color: C.muted, charSpacing: 2 });
  const devs = [["monitor", "Компютри в кабинета"], ["laptop", "Лаптопи"], ["phone", "Телефони"]];
  for (let i = 0; i < 3; i++) {
    const y = 2.6 + i * 0.86;
    x.shape("dev", pres.ShapeType.roundRect, { x: 0.75, y, w: 3.3, h: 0.7, rectRadius: 0.1, fill: { color: C.card }, line: { color: C.line, width: 1 } });
    await x.icon("dev", devs[i][0], { x: 0.97, y: y + 0.19, size: 0.32 });
    x.text("dev", devs[i][1], { x: 1.45, y: y + 0.19, w: 2.5, h: 0.34, size: 16 });
  }
  x.text("dev", "WebGPU · WebAssembly · Web Worker", { x: 0.75, y: 5.22, w: 3.3, h: 0.3, size: 12.5, color: C.amber });
  // arrows
  x.shape("ar1", pres.ShapeType.line, { x: 4.2, y: 3.65, w: 1.05, h: 0, line: { color: C.amber, width: 2, endArrowType: "triangle", beginArrowType: "triangle" } });
  x.text("ar1", "WebSocket", { x: 4.05, y: 3.2, w: 1.4, h: 0.3, size: 13, bold: true, color: C.amber, align: "center" });
  x.text("ar1", "части\nрезултати\nсигнали", { x: 4.05, y: 3.82, w: 1.4, h: 0.8, size: 11, color: C.muted, align: "center" });
  // coordinator
  x.shape("co", pres.ShapeType.roundRect, { x: 5.4, y: 2.2, w: 3.95, h: 3.45, rectRadius: 0.16, fill: { color: "18140D" }, line: { color: C.amber, width: 1.5 } });
  x.text("co", "Координатор", { x: 5.7, y: 2.42, w: 3.4, h: 0.45, size: 22, bold: true });
  x.text("co", "Rust · tokio · axum", { x: 5.7, y: 2.9, w: 3.4, h: 0.3, size: 13, color: C.muted });
  const mods = ["Опашка с наеми", "Проверка", "Обединяване", "REST · JSON"];
  mods.forEach((m, i) => {
    const mx = 5.7 + (i % 2) * 1.72, my = 3.45 + Math.floor(i / 2) * 0.98;
    x.shape(`mod${i}`, pres.ShapeType.roundRect, { x: mx, y: my, w: 1.6, h: 0.82, rectRadius: 0.1, fill: { color: C.card2 }, line: { color: "3A3328", width: 1 } });
    x.text(`mod${i}`, m, { x: mx + 0.1, y: my, w: 1.4, h: 0.82, size: 13.5, font: F.m, align: "center", valign: "middle" });
  });
  x.shape("ar2", pres.ShapeType.line, { x: 9.5, y: 3.65, w: 0.85, h: 0, line: { color: C.amber, width: 2, endArrowType: "triangle", beginArrowType: "triangle" } });
  x.text("ar2", "SQL", { x: 9.45, y: 3.2, w: 0.95, h: 0.3, size: 13, bold: true, color: C.amber, align: "center" });
  x.shape("db", pres.ShapeType.can, { x: 10.6, y: 2.75, w: 1.95, h: 1.85, fill: { color: C.card2 }, line: { color: C.blue, width: 1.5 } });
  x.text("db", "PostgreSQL", { x: 10.45, y: 3.55, w: 2.25, h: 0.4, size: 16, bold: true, align: "center" });
  x.text("db", "задачи · части\nрезултати · мрежи", { x: 10.45, y: 4.75, w: 2.25, h: 0.6, size: 12, color: C.muted, align: "center" });
  x.shape("con", pres.ShapeType.line, { x: 7.375, y: 5.7, w: 0, h: 0.42, line: { color: C.amber, width: 2, beginArrowType: "triangle", endArrowType: "triangle" } });
  x.shape("con", pres.ShapeType.roundRect, { x: 5.4, y: 6.15, w: 3.95, h: 0.55, rectRadius: 0.1, fill: { color: C.card }, line: { color: C.line, width: 1 } });
  x.text("con", "Конзола и изглед на живо · SvelteKit", { x: 5.4, y: 6.15, w: 3.95, h: 0.55, size: 14, align: "center", valign: "middle" });
  x.text("note", "Една и съща логика на Rust проверява в координатора и изчислява в браузъра като WebAssembly.", { x: 10.0, y: 5.6, w: 2.6, h: 1.1, size: 12.5, color: C.muted, lineSpacingMultiple: 1.05 });
  x.anim("auto", [
    { g: "devH", fx: "fade", dur: 500, delay: 200 },
    { g: "dev", fx: "left", dur: 800, delay: 250 },
    { g: "ar1", fx: "wipe", dur: 600, delay: 900 },
    { g: "co", fx: "zoom", dur: 800, delay: 1300 },
    ...[0, 1, 2, 3].map((i) => ({ g: `mod${i}`, fx: "pop", dur: 600, delay: 1700 + i * 120 })),
    { g: "ar2", fx: "wipe", dur: 600, delay: 2300 },
    { g: "db", fx: "right", dur: 800, delay: 2600 },
    { g: "con", fx: "float", dur: 800, delay: 3000 },
    { g: "note", fx: "fade", dur: 800, delay: 3400 },
  ]);
  x.notes(7);
}

// 9 — Determinism
{
  const x = newSlide(L("blue"), { transition: { type: "morph", dur: 1200 } });
  x.eyebrow("ДЕТЕРМИНИЗЪМ");
  x.title("Еднакво до последния бит.\nНа всяка видеокарта.");
  const browsers = [["Chrome", "Tint"], ["Edge", "Tint"], ["Safari", "WebKit"], ["Firefox", "naga"]];
  browsers.forEach(([b, c], i) => {
    const y = 2.75 + i * 0.85;
    x.shape(`br${i}`, pres.ShapeType.roundRect, { x: 0.75, y, w: 2.75, h: 0.66, rectRadius: 0.1, fill: { color: C.card }, line: { color: C.line, width: 1 } });
    x.text(`br${i}`, b, { x: 0.98, y, w: 1.4, h: 0.66, size: 17, bold: true, valign: "middle" });
    x.text(`br${i}`, c, { x: 2.2, y, w: 1.15, h: 0.66, size: 13, color: C.muted, valign: "middle", align: "right" });
    const ly = y + 0.33;
    x.shape(`br${i}`, pres.ShapeType.line, { x: 3.55, y: Math.min(ly, 3.85), w: 0.9, h: Math.max(Math.abs(3.85 - ly), 0.001), line: { color: C.blue, width: 1.25, transparency: 30 }, flipV: ly > 3.85 });
  });
  x.shape("same", pres.ShapeType.roundRect, { x: 4.45, y: 3.4, w: 2.15, h: 0.9, rectRadius: 0.12, fill: { color: "0E1A2A" }, line: { color: C.blue, width: 1.5 } });
  x.text("same", "Един и същ\nрезултат", { x: 4.45, y: 3.4, w: 2.15, h: 0.9, size: 15, bold: true, align: "center", valign: "middle", lineSpacingMultiple: 0.95 });
  x.text("cap", "Три компилатора на шейдъри, една видеокарта (Apple M1 Max)", { x: 0.75, y: 6.2, w: 6, h: 0.3, size: 12, color: C.faint });
  const why = [
    ["layers", "Целочислена аритметика", "Невронните мрежи и разписанията се смятат с цели числа — няма закръгляне, което да се различава."],
    ["key", "Случайни числа от брояч", "Всяка проба знае своите случайни числа, на всяко устройство, без общо състояние."],
  ];
  for (let i = 0; i < 2; i++) {
    const y = 2.5 + i * 1.42;
    x.shape(`why${i}`, pres.ShapeType.ellipse, { x: 7.25, y, w: 0.56, h: 0.56, fill: { color: C.blue, transparency: 84 }, line: { type: "none" } });
    await x.icon(`why${i}`, why[i][0], { x: 7.37, y: y + 0.12, size: 0.32, color: C.blue });
    x.text(`why${i}`, why[i][1], { x: 8.05, y: y + 0.02, w: 4.55, h: 0.4, size: 19, bold: true });
    x.text(`why${i}`, why[i][2], { x: 8.05, y: y + 0.44, w: 4.55, h: 0.75, size: 15, color: C.muted, lineSpacingMultiple: 1.05 });
  }
  x.text("rngH", "ИЗМЕРИХ И ГЕНЕРАТОРА", { x: 7.25, y: 5.3, w: 5, h: 0.28, size: 12, bold: true, color: C.muted, charSpacing: 2 });
  x.shape("rng1", pres.ShapeType.roundRect, { x: 7.25, y: 5.68, w: 2.6, h: 0.72, rectRadius: 0.1, fill: { color: "2A1213" }, line: { color: C.red, width: 1 } });
  x.text("rng1", [{ text: "z = +7,8", options: { bold: true, color: C.red, breakLine: true } }, { text: "отместен → отхвърлен", options: { color: C.muted, fontSize: 12 } }], { x: 7.4, y: 5.72, w: 2.4, h: 0.66, size: 15, valign: "middle" });
  x.shape("rng2", pres.ShapeType.roundRect, { x: 10.0, y: 5.68, w: 2.6, h: 0.72, rectRadius: 0.1, fill: { color: "0F2419" }, line: { color: C.green, width: 1 } });
  x.text("rng2", [{ text: "z = +0,4", options: { bold: true, color: C.green, breakLine: true } }, { text: "избран, 400 пускания", options: { color: C.muted, fontSize: 12 } }], { x: 10.15, y: 5.72, w: 2.4, h: 0.66, size: 15, valign: "middle" });
  x.anim("auto", [
    ...[0, 1, 2, 3].map((i) => ({ g: `br${i}`, fx: "left", dur: 700, delay: 200 + i * 150 })),
    { g: "same", fx: "zoom", dur: 800, delay: 1000 },
    { g: "cap", fx: "fade", dur: 600, delay: 1300 },
    { g: "why0", fx: "float", dur: 800, delay: 1400 },
    { g: "why1", fx: "float", dur: 800, delay: 1650 },
    { g: "rngH", fx: "fade", dur: 600, delay: 2000 },
    { g: "rng1", fx: "float", dur: 700, delay: 2150 },
    { g: "rng2", fx: "float", dur: 700, delay: 2450 },
  ]);
  x.notes(8);
}

// 10 — Verification
{
  const x = newSlide(L("red"), { transition: { type: "morph", dur: 1200 } });
  x.eyebrow("ПРОВЕРКА");
  x.title("Всеки може да се включи.\nЗатова всеки резултат се проверява.");
  const layers = [
    ["copy", "Две копия", "Всяка част се изчислява от две различни устройства."],
    ["target", "Контролни части", "Части с известен отговор, неразличими от останалите."],
    ["users", "Репутация", "На доказалите се устройства се вярва. Измамниците се блокират."],
  ];
  for (let i = 0; i < 3; i++) {
    const y = 2.6 + i * 1.3;
    x.shape(`ly${i}`, pres.ShapeType.roundRect, { x: 0.75, y, w: 5.75, h: 1.15, rectRadius: 0.12, fill: { color: C.card }, line: { color: C.line, width: 1 } });
    x.text(`ly${i}`, String(i + 1), { x: 0.98, y, w: 0.4, h: 1.15, size: 26, font: F.l, color: C.amber, valign: "middle" });
    await x.icon(`ly${i}`, layers[i][0], { x: 1.45, y: y + 0.4, size: 0.34 });
    x.text(`ly${i}`, layers[i][1], { x: 2.0, y: y + 0.14, w: 4.3, h: 0.36, size: 18, bold: true });
    x.text(`ly${i}`, layers[i][2], { x: 2.0, y: y + 0.5, w: 4.35, h: 0.6, size: 13.5, color: C.muted, lineSpacingMultiple: 1.0 });
  }
  x.text("s1", "3,4 s", { x: 7.0, y: 2.6, w: 2.7, h: 0.95, size: 58, font: F.l, color: C.red });
  x.text("s1", "до блокиране на измамник", { x: 7.0, y: 3.55, w: 2.7, h: 0.6, size: 14.5, color: C.muted });
  x.text("s2", "~70 s", { x: 9.9, y: 2.6, w: 2.7, h: 0.95, size: 58, font: F.l, color: C.red });
  x.text("s2", "за двама в сговор; 49 части — преизчислени за 11 s", { x: 9.9, y: 3.55, w: 2.7, h: 0.65, size: 14.5, color: C.muted });
  x.chart("cost", pres.ChartType.bar, [{ name: "Изчисления на част", labels: ["Наивно двойно", "Рояк"], values: [2.0, 1.26] }], {
    x: 6.85, y: 4.45, w: 5.8, h: 1.75, barDir: "bar", chartColors: [C.faint, C.amber], catAxisOrientation: "maxMin",
    showValue: true, dataLabelPosition: "outEnd", dataLabelColor: C.text, dataLabelFontSize: 13, dataLabelFontFace: F.h, dataLabelFormatCode: "0.00",
    catAxisLabelColor: C.muted, catAxisLabelFontSize: 13, catAxisLabelFontFace: F.h, catAxisLineShow: false,
    valAxisHidden: true, valAxisMinVal: 0, valAxisMaxVal: 2.4, valGridLine: { style: "none" }, catGridLine: { style: "none" },
    barGapWidthPct: 45, showLegend: false, showTitle: true, title: "Изчисления на част (по-малко е по-евтино)", titleColor: C.muted, titleFontSize: 12, titleFontFace: F.h,
    varyColors: true,
  });
  x.text("src", "Подход: D. P. Anderson, BOINC (2020); L. F. G. Sarmenta (2002). Рояк: 1,20–1,26 при 400 части.", { x: 6.85, y: 6.3, w: 5.8, h: 0.4, size: 11, color: C.faint });
  x.anim("auto", [
    ...[0, 1, 2].map((i) => ({ g: `ly${i}`, fx: "left", dur: 800, delay: 300 + i * 250 })),
    { g: "s1", fx: "float", dur: 800, delay: 1200 },
    { g: "s2", fx: "float", dur: 800, delay: 1450 },
    { g: "cost", fx: "wipe", dur: 1100, delay: 1800 },
    { g: "src", fx: "fade", dur: 600, delay: 2300 },
  ]);
  x.notes(9);
}

// ================================================================= SECTION: Доказателства
setSection("Доказателства");

const PC = (i) => `!!pc${String(i).padStart(2, "0")}`;

// 11 — The school lab
{
  const x = newSlide(L("bottomLeft"), { transition: { type: "morph", dur: 1200 } });
  x.eyebrow("ИЗПИТАНО В УЧИЛИЩЕ");
  x.title("13 стари компютъра.\nЕдин училищен кабинет.");
  for (let i = 0; i < 13; i++) x.hex(PC(i), { cx: 0.97 + i * 0.5, cy: 2.75, r: 0.2, fill: C.blue, t: 10 });
  x.text("sub", "Intel i3 от 2011–2012 г., без WebGPU — изчисляват на процесора. 4 ноември 2026 г.", { x: 7.65, y: 2.6, w: 5.0, h: 0.6, size: 14, color: C.muted });
  const stats = [
    ["70", " s", "картина 1920×1080 с 1 024 проби на пиксел: 2,1 милиарда светлинни пътя, 0 разминавания"],
    ["91", " s", "672 обучения на невронни мрежи, с еднаква класация на всеки компютър"],
    ["93,7", " %", "точност на най-добрата от 240 мрежи върху непознати ръкописни цифри (MNIST); всички 240 за 112 s"],
  ];
  stats.forEach(([n, u, l], i) => {
    const cx = 0.75 + i * 4.1;
    x.text(`st${i}`, [{ text: n }, { text: u, options: { color: C.amber, fontSize: 40 } }], { x: cx, y: 3.45, w: 3.8, h: 1.35, size: 84, font: F.l, color: C.text });
    x.text(`st${i}`, l, { x: cx, y: 4.85, w: 3.55, h: 1.3, size: 16, color: C.muted, lineSpacingMultiple: 1.08 });
  });
  x.anim("auto", [
    { g: "sub", fx: "fade", dur: 700, delay: 300 },
    ...[0, 1, 2].map((i) => ({ g: `st${i}`, fx: "float", dur: 900, delay: 600 + i * 260 })),
  ]);
  x.notes(10);
}

// 12 — Scale: the 13 cells sink into a field of a thousand
{
  const x = newSlide(L("blue"), { transition: { type: "morph", dur: 1600 } });
  x.eyebrow("МАЩАБ");
  x.title("От един телефон до хиляда устройства.");
  x.chart("bench", pres.ChartType.bar, [{
    name: "Милиарда проби за π в секунда",
    labels: ["Компютър в кабинета (i3, процесор)", "Телефон (Adreno 642L)", "MacBook M1 Max · Chrome", "MacBook M1 Max · Safari (WebKit)"],
    values: [0.37, 0.8, 4.13, 7.41],
  }], {
    x: 0.6, y: 2.2, w: 6.4, h: 4.2, barDir: "bar", chartColors: [C.amber],
    showValue: true, dataLabelPosition: "outEnd", dataLabelColor: C.text, dataLabelFontSize: 13, dataLabelFontFace: F.h, dataLabelFormatCode: "0.00",
    catAxisLabelColor: C.muted, catAxisLabelFontSize: 12, catAxisLabelFontFace: F.h, catAxisOrientation: "maxMin", catAxisLineShow: false,
    valAxisHidden: true, valAxisMinVal: 0, valAxisMaxVal: 9, valGridLine: { style: "none" }, catGridLine: { style: "none" },
    barGapWidthPct: 55, showLegend: false, showTitle: true, title: "Милиарда проби за π в секунда, едно устройство", titleColor: C.muted, titleFontSize: 12, titleFontFace: F.h,
  });
  const fw = 5.3, fh = fw * (SIZES.hexfield[1] / SIZES.hexfield[0]);
  x.image("field", { path: A.hexfield, x: 7.35, y: 2.2, w: fw, h: fh, transparency: 35, altText: "Хиляда клетки: хиляда симулирани устройства" });
  for (let i = 0; i < 13; i++) x.hex(PC(i), { cx: 7.48 + (i % 7) * 0.1395 * 1.5, cy: 2.33 + Math.floor(i / 7) * 0.121 * 2 + (i % 2 ? 0.06 : 0), r: 0.06, fill: C.blue, t: 0 });
  x.text("big", "1 000", { x: 7.35, y: 4.8, w: 5.3, h: 1.0, size: 64, font: F.l, color: C.text });
  x.text("big", "симулирани устройства едновременно: ~1 000 резултата в секунда, 0 отпаднали", { x: 7.35, y: 5.75, w: 5.3, h: 0.65, size: 15, color: C.muted });
  x.text("net", "Публичен адрес: ~20 MB/s ≈ 35 телефона на пълна скорост", { x: 7.35, y: 6.42, w: 5.3, h: 0.3, size: 12.5, color: C.amber });
  x.anim("auto", [
    { g: "bench", fx: "wipe", dur: 1200, delay: 300 },
    { g: "field", fx: "fade", dur: 1500, delay: 400 },
    { g: "big", fx: "float", dur: 900, delay: 1100 },
    { g: "net", fx: "fade", dur: 700, delay: 1600 },
  ]);
  x.notes(11);
}

// 13 — Safety and security
{
  const x = newSlide(L("topRight"), { transition: { type: "morph", dur: 1200 } });
  x.eyebrow("БЕЗОПАСНОСТ");
  x.title("Устройството остава на доброволеца.");
  const cards = [
    ["gauge", "75 % мощност", "По подразбиране. Доброволецът избира от 25 до 100 %."],
    ["eyeOff", "Пауза при скрита страница", "Освен ако е отворена изрично за работа във фонов режим."],
    ["battery", "Пауза под 30 % батерия", "В Chrome и Edge, които съобщават заряда на батерията."],
    ["stop", "„Спри“ спира всичко", "Частта се връща в опашката без резултат и се изчислява от друг."],
    ["lock", "Само известен код", "Ядрата се проверяват по хеш. Програмите нямат мрежа, файлове и часовник."],
    ["key", "Защитен сървър", "Пароли с Argon2id, ограничени опити за вход, CSP и HSTS, роли и покани."],
  ];
  for (let i = 0; i < 6; i++) {
    const cx = 0.75 + (i % 3) * 4.05, cy = 2.35 + Math.floor(i / 3) * 2.15;
    const [icon, head, body] = cards[i];
    x.shape(`c${i}`, pres.ShapeType.roundRect, { x: cx, y: cy, w: 3.75, h: 1.95, rectRadius: 0.14, fill: { color: C.card }, line: { color: C.line, width: 1 } });
    x.shape(`c${i}`, pres.ShapeType.ellipse, { x: cx + 0.28, y: cy + 0.28, w: 0.54, h: 0.54, fill: { color: i >= 4 ? C.green : C.amber, transparency: 84 }, line: { type: "none" } });
    await x.icon(`c${i}`, icon, { x: cx + 0.39, y: cy + 0.39, size: 0.32, color: i >= 4 ? C.green : C.amber });
    x.text(`c${i}`, head, { x: cx + 0.98, y: cy + 0.33, w: 2.6, h: 0.45, size: 17, bold: true, valign: "middle" });
    x.text(`c${i}`, body, { x: cx + 0.28, y: cy + 1.0, w: 3.25, h: 0.8, size: 14, color: C.muted, lineSpacingMultiple: 1.04 });
  }
  x.anim("auto", [0, 1, 2, 3, 4, 5].map((i) => ({ g: `c${i}`, fx: "float", dur: 800, delay: 300 + i * 140 })));
  x.notes(12);
}

// 14 — AI on the swarm
{
  const x = newSlide(L("center"), { transition: { type: "morph", dur: 1200 } });
  x.eyebrow("ИЗКУСТВЕН ИНТЕЛЕКТ");
  x.title("Езиков модел във всеки браузър.\nБез облак.");
  const flow = [
    ["file", "Файл с въпроси", "разделен на части"],
    ["phone", "Всяко устройство с видеокарта", "изпълнява целия модел: SmolLM2, 360 млн. параметъра"],
    ["copy", "Две устройства на въпрос", "отговорите се сравняват"],
  ];
  for (let i = 0; i < 3; i++) {
    const y = 2.65 + i * 0.86;
    x.shape(`fl${i}`, pres.ShapeType.ellipse, { x: 0.75, y, w: 0.56, h: 0.56, fill: { color: C.amber, transparency: 84 }, line: { type: "none" } });
    await x.icon(`fl${i}`, flow[i][0], { x: 0.87, y: y + 0.12, size: 0.32 });
    x.text(`fl${i}`, [{ text: flow[i][1], options: { bold: true, breakLine: true } }, { text: flow[i][2], options: { color: C.muted, fontSize: 14 } }], { x: 1.55, y: y - 0.04, w: 5.2, h: 0.7, size: 17, lineSpacingMultiple: 1.0 });
    if (i < 2) x.shape(`fl${i}`, pres.ShapeType.line, { x: 1.03, y: y + 0.6, w: 0, h: 0.22, line: { color: C.amber, width: 1.25, transparency: 40 } });
  }
  x.text("s1", "30 от 30", { x: 7.3, y: 2.5, w: 5.3, h: 0.95, size: 60, font: F.l, color: C.amber });
  x.text("s1", "отговора на телефона съвпадат с тези на лаптопа", { x: 7.3, y: 3.45, w: 5.3, h: 0.4, size: 15, color: C.muted });
  x.text("s2", [{ text: "~66" }, { text: " токена/s", options: { fontSize: 26, color: C.muted } }], { x: 7.3, y: 4.0, w: 5.3, h: 0.85, size: 48, font: F.l, color: C.text });
  x.text("s2", "на лаптопа: 50 въпроса за около 24 s", { x: 7.3, y: 4.85, w: 5.3, h: 0.4, size: 15, color: C.muted });
  x.shape("story", pres.ShapeType.roundRect, { x: 0.75, y: 5.45, w: 11.85, h: 1.15, rectRadius: 0.12, fill: { color: "1F1A10" }, line: { color: "5A4419", width: 1 } });
  await x.icon("story", "alert", { x: 1.0, y: 5.82, size: 0.4 });
  x.text("story", [
    { text: "Телефонът отговаряше с безсмислици. ", options: { bold: true, color: C.text } },
    { text: "Причината: слято изчислително ядро в ONNX Runtime смята грешно на видеокартата Adreno. Сега всяко устройство първо решава задача с известен отговор — ако сгреши, не получава работа.", options: { color: C.muted } },
  ], { x: 1.6, y: 5.45, w: 10.8, h: 1.15, size: 15, valign: "middle", lineSpacingMultiple: 1.05 });
  x.anim("auto", [
    ...[0, 1, 2].map((i) => ({ g: `fl${i}`, fx: "left", dur: 700, delay: 300 + i * 230 })),
    { g: "s1", fx: "float", dur: 900, delay: 1100 },
    { g: "s2", fx: "float", dur: 900, delay: 1350 },
  ]);
  x.anim("click", [{ g: "story", fx: "float", dur: 800, delay: 0 }]);
  x.notes(13);
}

// 15 — Engineering quality
{
  const x = newSlide(L("topRight"), { transition: { type: "morph", dur: 1200 } });
  x.eyebrow("КАЧЕСТВО");
  x.title("Доказано с тестове и измервания.");
  const stats = [
    ["287", "теста на Rust, много от тях срещу истинска PostgreSQL"],
    ["120", "модулни теста на интерфейса (Vitest)"],
    ["3", "браузърни двигателя в тестовете от край до край: Chromium, Firefox, WebKit"],
    ["17", "спецификации преди кода, всяка с критерии за приемане"],
  ];
  stats.forEach(([n, l], i) => {
    const cx = 0.75 + i * 3.05;
    x.text(`q${i}`, n, { x: cx, y: 2.35, w: 2.8, h: 1.15, size: 72, font: F.l, color: i === 0 ? C.amber : C.text });
    x.text(`q${i}`, l, { x: cx, y: 3.5, w: 2.7, h: 1.0, size: 14.5, color: C.muted, lineSpacingMultiple: 1.05 });
  });
  const stack = ["Rust", "tokio · axum", "PostgreSQL", "WebGPU · WGSL", "WebAssembly", "TypeScript", "SvelteKit", "Docker", "GitHub Actions", "Caddy · TLS", "GPL-3.0"];
  let px = 0.75, py = 4.9;
  stack.forEach((t) => {
    const w = 0.42 + t.length * 0.105;
    if (px + w > 12.6) { px = 0.75; py += 0.58; }
    x.shape("stack", pres.ShapeType.roundRect, { x: px, y: py, w, h: 0.44, rectRadius: 0.22, fill: { color: C.card2 }, line: { color: C.line, width: 1 } });
    x.text("stack", t, { x: px, y: py, w, h: 0.44, size: 13, font: F.m, align: "center", valign: "middle" });
    px += w + 0.15;
  });
  x.text("foot", "≈ 22 000 реда на Rust · ≈ 20 000 на TypeScript и Svelte · CI при всяко качване · всяко число от тази презентация е в docs/measurements.md", { x: 0.75, y: 6.2, w: 11.85, h: 0.4, size: 12.5, color: C.faint });
  x.anim("auto", [
    ...[0, 1, 2, 3].map((i) => ({ g: `q${i}`, fx: "float", dur: 800, delay: 300 + i * 180 })),
    { g: "stack", fx: "fade", dur: 900, delay: 1200 },
    { g: "foot", fx: "fade", dur: 700, delay: 1600 },
  ]);
  x.notes(14);
}

// ================================================================= SECTION: Напред
setSection("Напред");

// 16 — Roadmap
{
  const x = newSlide(L("bottomLeft"), { transition: { type: "morph", dur: 1200 } });
  x.eyebrow("НАПРЕД");
  x.title("Следващите стъпки");
  x.shape("line", pres.ShapeType.line, { x: 0.89, y: 3.0, w: 11.4, h: 0, line: { color: C.line, width: 2 } });
  x.shape("done", pres.ShapeType.line, { x: 0.89, y: 3.0, w: 3.05, h: 0, line: { color: C.amber, width: 2.5 } });
  const steps = [
    ["Окт – ное 2026", "Изпълнено", "Координатор, проверка, мрежи, разписания, невронни мрежи, собствени програми, езиков модел, изпитване в кабинет."],
    ["Дек 2026 – яну 2027", "Реч и език", "Реч в браузъра (Whisper): урок, преписан от залата. Модел на български. Разписание с две смени."],
    ["Фев 2027", "Кабинетът", "Локален възел за кабинета, вход с училищен акаунт, режим „киоск“, крива на ускорението."],
    ["Мар – апр 2027", "Мащаб", "Изпитване в няколко училища."],
  ];
  steps.forEach(([when, head, body], i) => {
    const cx = 0.75 + i * 3.05;
    x.hex(`n${i}`, { cx: cx + 0.14, cy: 3.0, r: 0.15, fill: i === 0 ? C.amber : C.card2, t: 0, line: i === 0 ? null : C.amber, lineW: 1.5 });
    x.text(`n${i}`, when, { x: cx, y: 3.4, w: 2.8, h: 0.35, size: 14, bold: true, color: C.amber });
    x.text(`n${i}`, head, { x: cx, y: 3.8, w: 2.8, h: 0.45, size: 21, bold: true });
    x.text(`n${i}`, body, { x: cx, y: 4.35, w: 2.75, h: 1.9, size: 15, color: C.muted, lineSpacingMultiple: 1.08 });
  });
  x.anim("auto", [
    { g: "line", fx: "wipe", dur: 1200, delay: 200 },
    { g: "done", fx: "wipe", dur: 900, delay: 700 },
    ...[0, 1, 2, 3].map((i) => ({ g: `n${i}`, fx: "float", dur: 800, delay: 600 + i * 300 })),
  ]);
  x.notes(15);
}

// 17 — Closing: the swarm lights up
{
  const x = newSlide("Hero", { transition: { type: "fade", dur: 1200 } });
  x.image("glow", { path: A["glow-amber"], x: 6.25, y: 0.25, w: 7, h: 7, transparency: 15 });
  swarm(x, patchXY(9.75, 3.75, 0.74), { r: 0.69, mono: C.amber, monoT: (i) => (i === 0 ? 0 : i <= 6 ? 8 : 22) });
  x.text("kicker", "БЛАГОДАРЯ", { x: 0.75, y: 1.15, w: 6, h: 0.32, size: 12.5, bold: true, color: C.amber, charSpacing: 2.5 });
  x.text("l1", "Всяко училище вече\nима суперкомпютър.", { x: 0.72, y: 1.6, w: 5.95, h: 1.5, size: 40, bold: true, lineSpacingMultiple: 0.95 });
  x.text("l2", "Просто още не го е включило.", { x: 0.75, y: 3.35, w: 6.4, h: 0.6, size: 28, font: F.l, color: C.amber });
  x.shape("qr", pres.ShapeType.roundRect, { x: 0.75, y: 4.6, w: 1.65, h: 1.65, rectRadius: 0.12, fill: { color: "FFFFFF" }, line: { type: "none" } });
  x.image("qr", { path: A.qr, x: 0.83, y: 4.68, w: 1.49, h: 1.49, altText: "QR код към swarmrelay.duckdns.org" });
  x.text("qr", [{ text: "Опитайте сами", options: { bold: true, breakLine: true, color: C.text } }, { text: "swarmrelay.duckdns.org", options: { color: C.amber, breakLine: true } }, { text: "Отворен код · GPL-3.0", options: { color: C.muted } }], { x: 2.65, y: 4.75, w: 4.4, h: 1.3, size: 17, lineSpacingMultiple: 1.25 });
  x.anim("auto", [
    { g: "glow", fx: "fade", dur: 1800, delay: 0 },
    ...ringOrder(100, 120),
    { g: "kicker", fx: "fade", dur: 700, delay: 400 },
    { g: "l1", fx: "float", dur: 1000, delay: 550 },
    { g: "l2", fx: "float", dur: 1000, delay: 1500 },
    { g: "qr", fx: "float", dur: 900, delay: 2200 },
  ]);
  x.notes(16);
}

// ================================================================= SECTION: Резерв
setSection("Резерв за въпросите");

const RESERVE = [
  [19, "Защо не BOINC?"],
  [20, "Помагат ли повече устройства на разписанието?"],
  [21, "Колко устройства издържа координаторът?"],
  [22, "Не прегряват ли телефоните?"],
  [23, "Как се сравняват отговори на езиков модел?"],
  [24, "REST програмният интерфейс"],
  [25, "Какво още не е готово? Разхищение на ток ли е?"],
];

// 18 — Reserve index
{
  const x = newSlide(L("plain"), { transition: { type: "fade", dur: 700 }, hidden: true });
  x.eyebrow("РЕЗЕРВ", C.muted);
  x.title("Въпроси");
  RESERVE.forEach(([n, q], i) => {
    const cx = 0.75 + (i % 2) * 6.05, cy = 2.4 + Math.floor(i / 2) * 1.0;
    x.shape(`r${i}`, pres.ShapeType.roundRect, { x: cx, y: cy, w: 5.8, h: 0.8, rectRadius: 0.1, fill: { color: C.card }, line: { color: C.line, width: 1 } });
    x.text(`r${i}`, String(n), { x: cx + 0.25, y: cy, w: 0.7, h: 0.8, size: 24, font: F.l, color: C.amber, valign: "middle" });
    x.text(`r${i}`, q, { x: cx + 1.0, y: cy, w: 4.6, h: 0.8, size: 16, valign: "middle" });
  });
  x.text("hint", "Въведете номера на слайда и натиснете Enter.", { x: 0.75, y: 6.45, w: 8, h: 0.3, size: 12, color: C.faint });
  x.notes(17);
}

// 19 — Why not BOINC
{
  const x = newSlide(L("plain"), { transition: { type: "fade", dur: 700 }, hidden: true });
  x.eyebrow("РЕЗЕРВ · 19", C.muted);
  x.title("Защо не BOINC?");
  const rows = [
    ["Включване", "Отваряне на страница или QR код", "Инсталиране с права на администратор"],
    ["Устройства", "Всеки браузър: заключени училищни компютри, телефони", "Устройства, на които може да се инсталира"],
    ["Изчисляване", "WebGPU и WebAssembly в браузъра", "Собствен клиент за процесор и видеокарта"],
    ["Задачи", "На самото училище: разписание, обучение, собствени програми", "Големи научни проекти"],
    ["Проверка", "Две копия, контролни части, репутация", "Повтаряне и репутация (Anderson, 2020)"],
  ];
  const tbl = [
    [
      { text: "", options: { fill: { color: C.bg } } },
      { text: "Рояк", options: { bold: true, color: C.amber, fill: { color: C.bg } } },
      { text: "BOINC, Folding@home", options: { bold: true, color: C.muted, fill: { color: C.bg } } },
    ],
    ...rows.map((r) => [
      { text: r[0], options: { bold: true, color: C.text } },
      { text: r[1], options: { color: C.text } },
      { text: r[2], options: { color: C.muted } },
    ]),
  ];
  x.s.addTable(tbl, {
    x: 0.75, y: 2.3, w: 11.85, colW: [2.2, 4.85, 4.8], rowH: 0.68, fontFace: F.h, fontSize: 15,
    fill: { color: C.card }, border: { type: "solid", pt: 0.75, color: C.line }, valign: "middle", margin: [0.06, 0.15, 0.06, 0.15],
    objectName: "tbl",
  });
  x.notes(18);
}

// 20 — Timetable: more searches, better timetables
{
  const x = newSlide(L("plain"), { transition: { type: "fade", dur: 700 }, hidden: true });
  x.eyebrow("РЕЗЕРВ · 20", C.muted);
  x.title("Повече търсения — по-добро разписание");
  x.chart("tt", pres.ChartType.line, [
    { name: "40 паралелки", labels: ["1", "2", "4", "8", "16", "32"], values: [60, 35, 35, 35, 35, 35] },
    { name: "60 паралелки", labels: ["1", "2", "4", "8", "16", "32"], values: [150, 150, 150, 125, 125, 125] },
  ], {
    x: 0.6, y: 2.2, w: 8.2, h: 4.4, chartColors: [C.amber, C.blue], lineSize: 3, lineDataSymbol: "circle", lineDataSymbolSize: 9,
    showValue: true, dataLabelPosition: "t", dataLabelColor: C.text, dataLabelFontSize: 12, dataLabelFontFace: F.h,
    catAxisLabelColor: C.muted, catAxisLabelFontSize: 12, catAxisLabelFontFace: F.h, catAxisTitle: "брой търсения (по едно на устройство)", showCatAxisTitle: true, catAxisTitleColor: C.muted, catAxisTitleFontSize: 12,
    valAxisLabelColor: C.muted, valAxisLabelFontSize: 11, valAxisLabelFontFace: F.h, valAxisMinVal: 0, valAxisMaxVal: 175,
    valGridLine: { color: "26262B", size: 0.75 }, catGridLine: { style: "none" },
    showLegend: true, legendPos: "t", legendColor: C.text, legendFontSize: 13, legendFontFace: F.h,
    showTitle: true, title: "Най-добра цена (по-малко е по-добре), 2 милиона стъпки на търсене", titleColor: C.muted, titleFontSize: 12, titleFontFace: F.h,
  });
  x.text("side", "Всяко търсене е различно, а координаторът пази най-доброто. Цената брои меките правила (прозорци, желания); всички намерени разписания са валидни.", { x: 9.2, y: 2.5, w: 3.4, h: 2.4, size: 16, color: C.muted, lineSpacingMultiple: 1.1 });
  x.text("side2", "Едно търсене на 40 паралелки: 0,40 s на едно ядро (0,49 s на M1 Max) — с еднакви цени.", { x: 9.2, y: 5.0, w: 3.4, h: 1.2, size: 14, color: C.faint, lineSpacingMultiple: 1.1 });
  x.notes(19);
}

// 21 — Coordinator capacity
{
  const x = newSlide(L("plain"), { transition: { type: "fade", dur: 700 }, hidden: true });
  x.eyebrow("РЕЗЕРВ · 21", C.muted);
  x.title("Капацитет на координатора");
  x.chart("cap", pres.ChartType.bar, [{ name: "Резултата в секунда", labels: ["10", "100", "300", "1 000"], values: [508, 564, 718, 841] }], {
    x: 0.6, y: 2.2, w: 7.2, h: 4.3, chartColors: [C.amber], barGapWidthPct: 60,
    showValue: true, dataLabelPosition: "outEnd", dataLabelColor: C.text, dataLabelFontSize: 13, dataLabelFontFace: F.h,
    catAxisLabelColor: C.muted, catAxisLabelFontSize: 13, catAxisLabelFontFace: F.h, catAxisTitle: "устройства, които отговарят веднага (най-тежкият случай)", showCatAxisTitle: true, catAxisTitleColor: C.muted, catAxisTitleFontSize: 12,
    valAxisHidden: true, valAxisMinVal: 0, valAxisMaxVal: 1000, valGridLine: { style: "none" }, catGridLine: { style: "none" },
    showLegend: false, showTitle: true, title: "Резултата в секунда, 0 отпаднали при всяко натоварване", titleColor: C.muted, titleFontSize: 12, titleFontFace: F.h,
  });
  const facts = [
    ["930–980", "резултата/s при 1 000 устройства с части от 1 s (идеално: 1 000)"],
    ["1 000–1 100", "резултата/s при насищане"],
    ["Таван", "заключването на реда на задачата при всяко завършване, не процесорът — затова частите носят секунди работа"],
  ];
  facts.forEach(([n, l], i) => {
    const y = 2.4 + i * 1.35;
    x.text(`f${i}`, n, { x: 8.3, y, w: 4.3, h: 0.6, size: 30, font: F.l, color: i === 2 ? C.text : C.amber });
    x.text(`f${i}`, l, { x: 8.3, y: y + 0.6, w: 4.3, h: 0.7, size: 13.5, color: C.muted });
  });
  x.notes(20);
}

// 22 — Contributor power, measured on a phone
{
  const x = newSlide(L("plain"), { transition: { type: "fade", dur: 700 }, hidden: true });
  x.eyebrow("РЕЗЕРВ · 22", C.muted);
  x.title("Мощността, измерена на телефон");
  x.chart("pw", pres.ChartType.bar, [{ name: "Дял от времето в изчисление", labels: ["25 %", "50 %", "75 % (по подразбиране)", "100 %"], values: [25, 50, 79, 98] }], {
    x: 0.6, y: 2.2, w: 7.4, h: 4.3, chartColors: [C.amber], barGapWidthPct: 60,
    showValue: true, dataLabelPosition: "outEnd", dataLabelColor: C.text, dataLabelFontSize: 13, dataLabelFontFace: F.h, dataLabelFormatCode: '0" %"',
    catAxisLabelColor: C.muted, catAxisLabelFontSize: 13, catAxisLabelFontFace: F.h, catAxisTitle: "избрана мощност", showCatAxisTitle: true, catAxisTitleColor: C.muted, catAxisTitleFontSize: 12,
    valAxisHidden: true, valAxisMinVal: 0, valAxisMaxVal: 110, valGridLine: { style: "none" }, catGridLine: { style: "none" },
    showLegend: false, showTitle: true, title: "Дял от времето, в което телефонът изчислява (Nothing Phone (1))", titleColor: C.muted, titleFontSize: 12, titleFontFace: F.h,
  });
  const facts = [
    ["10,6 min", "под пълно натоварване, без забавяне: 332 ms на част от първата до последната минута"],
    ["36–37 °C", "видеокартата през цялото време (на зарядно)"],
    ["< 30 %", "батерия: устройството спира само (Chrome и Edge)"],
  ];
  facts.forEach(([n, l], i) => {
    const y = 2.4 + i * 1.35;
    x.text(`f${i}`, n, { x: 8.5, y, w: 4.1, h: 0.6, size: 30, font: F.l, color: C.amber });
    x.text(`f${i}`, l, { x: 8.5, y: y + 0.6, w: 4.1, h: 0.7, size: 13.5, color: C.muted });
  });
  x.notes(21);
}

// 23 — Comparing language-model answers
{
  const x = newSlide(L("plain"), { transition: { type: "fade", dur: 700 }, hidden: true });
  x.eyebrow("РЕЗЕРВ · 23", C.muted);
  x.title("Как се сравняват отговори на езиков модел");
  const nums = [
    ["50 / 50", "еднакви отговора: Chrome и Edge (един компилатор)"],
    ["34 / 50", "еднакви: Chrome и Safari на същата видеокарта"],
    ["≤ 0,31", "разликата при всяко разделяне: почти равен избор, никога на първия токен"],
  ];
  nums.forEach(([n, l], i) => {
    const cx = 0.75 + i * 4.05;
    x.text(`n${i}`, n, { x: cx, y: 2.35, w: 3.8, h: 0.95, size: 50, font: F.l, color: C.amber });
    x.text(`n${i}`, l, { x: cx, y: 3.3, w: 3.6, h: 0.8, size: 15, color: C.muted });
  });
  x.shape("rule", pres.ShapeType.roundRect, { x: 0.75, y: 4.45, w: 11.85, h: 1.9, rectRadius: 0.14, fill: { color: C.card }, line: { color: C.line, width: 1 } });
  x.text("rule", [
    { text: "Правилото: ", options: { bold: true, color: C.text } },
    { text: "два отговора са еднакви до токен, който и двете устройства са избрали с разлика под 0,5 между първия и втория вариант — но не на първия токен. С точно съвпадение честен iPhone и честен телефон с Android щяха да се разминават в една трета от въпросите. При това правило и 150-те честни двойки съвпадат, а телефонът и лаптопът — в 30 от 30.", options: { color: C.muted } },
  ], { x: 1.05, y: 4.6, w: 11.25, h: 1.6, size: 15.5, valign: "middle", lineSpacingMultiple: 1.1 });
  x.notes(22);
}

// 24 — The REST API
{
  const x = newSlide(L("plain"), { transition: { type: "fade", dur: 700 }, hidden: true });
  x.eyebrow("РЕЗЕРВ · 24", C.muted);
  x.title("REST програмен интерфейс");
  const groups = [
    ["Задачи", ["POST /api/jobs", "GET  /api/jobs/{id}", "POST /api/jobs/{id}/cancel", "GET  /api/jobs/{id}/image.png"]],
    ["Мрежи", ["GET  /api/networks", "GET  /api/networks/{id}/devices", "POST /api/networks/{id}/invites", "GET  /api/networks/{id}/members"]],
    ["Профили и програми", ["POST /api/auth/login", "GET  /api/me", "GET  /api/programs/approved", "POST /api/admin/programs"]],
    ["WebSocket", ["/ws/worker", "/ws/networks/{id}/live", "/ws/join/{code}/summary", "/ws/dashboard"]],
  ];
  groups.forEach(([head, lines], i) => {
    const cx = 0.75 + (i % 2) * 6.05, cy = 2.2 + Math.floor(i / 2) * 2.05;
    x.shape(`g${i}`, pres.ShapeType.roundRect, { x: cx, y: cy, w: 5.8, h: 1.9, rectRadius: 0.12, fill: { color: C.card }, line: { color: C.line, width: 1 } });
    x.text(`g${i}`, head, { x: cx + 0.28, y: cy + 0.2, w: 5.2, h: 0.35, size: 15, bold: true, color: i === 3 ? C.blue : C.amber });
    x.text(`g${i}`, lines.join("\n"), { x: cx + 0.28, y: cy + 0.62, w: 5.3, h: 1.25, size: 13.5, font: "Menlo", color: C.text, lineSpacingMultiple: 1.12 });
  });
  x.text("foot", "JSON навсякъде. Типовете се генерират от Rust към TypeScript (ts-rs), а CI пада, ако са остарели.", { x: 0.75, y: 6.45, w: 11.85, h: 0.3, size: 12.5, color: C.faint });
  x.notes(23);
}

// 25 — Honest limits and energy
{
  const x = newSlide(L("plain"), { transition: { type: "fade", dur: 700 }, hidden: true });
  x.eyebrow("РЕЗЕРВ · 25", C.muted);
  x.title("Какво още предстои");
  const todo = [
    ["Кривата на ускорението", "спрямо един компютър в кабинета: имаме времена, не още базата"],
    ["iPhone", "Safari с WebGPU на истински телефон"],
    ["Данни на истинско училище", "разписанието върху плана на заместник-директор и върху XHSTT"],
    ["Свободното време на кабинета", "сега е оценка по разписанието; ще бъде измерено от самите компютри"],
  ];
  todo.forEach(([h, b], i) => {
    const y = 2.35 + i * 0.95;
    x.shape(`t${i}`, pres.ShapeType.ellipse, { x: 0.75, y: y + 0.08, w: 0.2, h: 0.2, fill: { color: C.card2 }, line: { color: C.amber, width: 1.5 } });
    x.text(`t${i}`, [{ text: h, options: { bold: true, breakLine: true } }, { text: b, options: { color: C.muted, fontSize: 14 } }], { x: 1.2, y, w: 6.0, h: 0.85, size: 17 });
  });
  x.shape("en", pres.ShapeType.roundRect, { x: 7.75, y: 2.35, w: 4.85, h: 3.6, rectRadius: 0.14, fill: { color: "0F2419" }, line: { color: "1F5A3A", width: 1 } });
  await x.icon("en", "leaf", { x: 8.05, y: 2.65, size: 0.42, color: C.green });
  x.text("en", "Разхищение на ток ли е?", { x: 8.05, y: 3.25, w: 4.3, h: 0.45, size: 19, bold: true });
  x.text("en", "Рояк използва компютри, които вече са включени: по подразбиране на 75 % мощност и само в работното време, което мрежата е задала. Телефонът спира сам под 30 % батерия.", { x: 8.05, y: 3.8, w: 4.3, h: 2.0, size: 15, color: C.muted, lineSpacingMultiple: 1.1 });
  x.notes(24);
}

// ---------------------------------------------------------------- write
if (notes.length !== SCRIPT.length) throw new Error(`script has ${SCRIPT.length} parts, deck has ${notes.length} slides`);
const raw = await pres.write({ outputType: "nodebuffer" });
fs.writeFileSync(DECK, await animate(raw, plans, THEME));

// The script, from the same text as the speaker notes.
let md = INTRO;
SCRIPT.forEach((p, i) => {
  if (i === 17) md += `\n## Резерв за въпросите\n`;
  md += `\n### ${i + 1}. [${p.time}]\n\n${p.say.replace(/\[ДЕЙСТВИЕ([^\]]*)\]/g, "**[ДЕЙСТВИЕ$1]**").replace(/\n/g, "\n\n")}\n`;
});
md += OUTRO;
fs.writeFileSync(path.join(OUT_DIR, "сценарий.md"), md);
console.log(`wrote ${DECK} (${plans.length} slides) and сценарий.md`);
