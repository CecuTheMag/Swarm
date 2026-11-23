// Generates every image the deck uses into ../build/assets: backgrounds, cropped
// screenshots, icons and the QR code. Run by build.mjs.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";
import QRCode from "qrcode";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import * as Lu from "react-icons/lu";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "../../..");
export const OUT = path.resolve(here, "../build/assets");
const SHOTS = path.join(repo, "docs/documentation/img");

const W = 2560;
const H = 1440;

// A faint flat-top honeycomb drawn as SVG paths, the motif of the product itself.
function honeycomb(r, opacity, stroke = "#ffffff") {
  const h = Math.sqrt(3) * r;
  let d = "";
  for (let col = -1; col * 1.5 * r < W + 2 * r; col++) {
    for (let row = -1; row * h < H + h; row++) {
      const cx = col * 1.5 * r;
      const cy = row * h + (col % 2 ? h / 2 : 0);
      const pts = [];
      for (let k = 0; k < 6; k++) {
        const a = (Math.PI / 3) * k;
        pts.push(`${(cx + r * Math.cos(a)).toFixed(1)},${(cy + r * Math.sin(a)).toFixed(1)}`);
      }
      d += `M${pts.join("L")}Z`;
    }
  }
  return `<path d="${d}" fill="none" stroke="${stroke}" stroke-opacity="${opacity}" stroke-width="1.6"/>`;
}

// Backgrounds: near-black with a soft amber (and sometimes blue) light, plus the
// honeycomb faded out by a radial mask so it only shows near the light.
const BACKGROUNDS = {
  hero: { glows: [[0.72, 0.42, 0.55, "#ffb53c", 0.30], [0.95, 0.95, 0.5, "#e0730b", 0.16]], comb: [0.72, 0.42, 0.62] },
  topRight: { glows: [[0.9, 0.05, 0.6, "#ffb53c", 0.17]], comb: [0.9, 0.05, 0.55] },
  bottomLeft: { glows: [[0.1, 1.0, 0.6, "#ffb53c", 0.15]], comb: [0.1, 1.0, 0.5] },
  center: { glows: [[0.5, 0.5, 0.55, "#ffb53c", 0.16]], comb: [0.5, 0.5, 0.6] },
  blue: { glows: [[0.85, 0.15, 0.55, "#4da3ff", 0.14], [0.1, 0.95, 0.45, "#ffb53c", 0.10]], comb: [0.85, 0.15, 0.5] },
  red: { glows: [[0.88, 0.2, 0.55, "#ff5b54", 0.12], [0.1, 0.95, 0.45, "#ffb53c", 0.10]], comb: [0.88, 0.2, 0.5] },
  plain: { glows: [[0.5, -0.2, 0.7, "#ffffff", 0.05]], comb: null },
};

function backgroundSvg({ glows, comb }) {
  const defs = glows
    .map(
      ([cx, cy, r, color, a], i) =>
        `<radialGradient id="g${i}" cx="${cx * W}" cy="${cy * H}" r="${r * W}" gradientUnits="userSpaceOnUse">
           <stop offset="0" stop-color="${color}" stop-opacity="${a}"/>
           <stop offset="0.45" stop-color="${color}" stop-opacity="${a * 0.35}"/>
           <stop offset="1" stop-color="${color}" stop-opacity="0"/>
         </radialGradient>`,
    )
    .join("");
  const combDefs = comb
    ? `<radialGradient id="cm" cx="${comb[0] * W}" cy="${comb[1] * H}" r="${comb[2] * W}" gradientUnits="userSpaceOnUse">
         <stop offset="0" stop-color="#fff" stop-opacity="1"/><stop offset="1" stop-color="#fff" stop-opacity="0"/>
       </radialGradient>
       <mask id="m"><rect width="${W}" height="${H}" fill="url(#cm)"/></mask>`
    : "";
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}">
    <defs>${defs}${combDefs}
      <linearGradient id="v" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.35"/>
      </linearGradient>
    </defs>
    <rect width="${W}" height="${H}" fill="#0a0a0c"/>
    ${glows.map((_, i) => `<rect width="${W}" height="${H}" fill="url(#g${i})"/>`).join("")}
    ${comb ? `<g mask="url(#m)">${honeycomb(64, 0.075)}</g>` : ""}
    <rect width="${W}" height="${H}" fill="url(#v)"/>
  </svg>`;
}

// A soft glow placed behind the hero honeycomb or a highlighted number.
function glowSvg(color, size = 1200) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}">
    <defs><radialGradient id="g" cx="50%" cy="50%" r="50%">
      <stop offset="0" stop-color="${color}" stop-opacity="0.55"/>
      <stop offset="0.5" stop-color="${color}" stop-opacity="0.16"/>
      <stop offset="1" stop-color="${color}" stop-opacity="0"/>
    </radialGradient></defs>
    <rect width="${size}" height="${size}" fill="url(#g)"/></svg>`;
}

// The product's logo: three honey cells, as in web/src/lib/ui/Logo.svelte's spirit.
function logoSvg() {
  const hex = (cx, cy, r, fill) => {
    const pts = [];
    for (let k = 0; k < 6; k++) {
      const a = (Math.PI / 3) * k + Math.PI / 6;
      pts.push(`${(cx + r * Math.cos(a)).toFixed(2)},${(cy + r * Math.sin(a)).toFixed(2)}`);
    }
    return `<polygon points="${pts.join(" ")}" fill="${fill}"/>`;
  };
  const r = 120;
  const w = Math.sqrt(3) * r;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">
    ${hex(256, 150, r - 8, "#f5a524")}
    ${hex(256 - w / 2, 150 + 1.5 * r, r - 8, "#f5a524")}
    ${hex(256 + w / 2, 150 + 1.5 * r, r - 8, "#ffc76a")}
  </svg>`;
}

function hexfieldSvg() {
  const cols = 50;
  const rows = 20;
  const r = 15;
  const w = 1.5 * r * cols + r;
  const h = Math.sqrt(3) * r * (rows + 0.5);
  let seed = 7;
  const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
  let cells = "";
  for (let c = 0; c < cols; c++) {
    for (let rr = 0; rr < rows; rr++) {
      const cx = r + 1.5 * r * c;
      const cy = Math.sqrt(3) * r * (rr + 0.5) + (c % 2 ? (Math.sqrt(3) * r) / 2 : 0);
      const pts = [];
      for (let k = 0; k < 6; k++) {
        const a = (Math.PI / 3) * k;
        pts.push(`${(cx + (r - 2.2) * Math.cos(a)).toFixed(1)},${(cy + (r - 2.2) * Math.sin(a)).toFixed(1)}`);
      }
      const blue = rnd() < 0.3;
      const a = 0.35 + rnd() * 0.65;
      cells += `<polygon points="${pts.join(" ")}" fill="${blue ? "#4da3ff" : "#ffb53c"}" fill-opacity="${a.toFixed(2)}"/>`;
    }
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${Math.ceil(w)}" height="${Math.ceil(h)}">${cells}</svg>`;
}

const ICONS = {
  scan: Lu.LuScanLine,
  cpu: Lu.LuCpu,
  check: Lu.LuShieldCheck,
  gauge: Lu.LuGauge,
  eyeOff: Lu.LuEyeOff,
  battery: Lu.LuBatteryLow,
  stop: Lu.LuCircleStop,
  lock: Lu.LuLock,
  box: Lu.LuBox,
  key: Lu.LuKeyRound,
  copy: Lu.LuCopy,
  target: Lu.LuTarget,
  users: Lu.LuUsers,
  zap: Lu.LuZap,
  brain: Lu.LuBrain,
  calendar: Lu.LuCalendarDays,
  code: Lu.LuCode,
  image: Lu.LuImage,
  sparkles: Lu.LuSparkles,
  server: Lu.LuServer,
  database: Lu.LuDatabase,
  monitor: Lu.LuMonitor,
  laptop: Lu.LuLaptop,
  phone: Lu.LuSmartphone,
  school: Lu.LuSchool,
  mic: Lu.LuMic,
  message: Lu.LuMessageSquareText,
  flask: Lu.LuFlaskConical,
  globe: Lu.LuGlobe,
  test: Lu.LuTestTubeDiagonal,
  git: Lu.LuGitBranch,
  file: Lu.LuFileText,
  scale: Lu.LuScale,
  plug: Lu.LuPlug,
  x: Lu.LuX,
  checkmark: Lu.LuCheck,
  network: Lu.LuNetwork,
  layers: Lu.LuLayers,
  alert: Lu.LuTriangleAlert,
  clock: Lu.LuClock,
  leaf: Lu.LuLeaf,
};

async function icon(name, color) {
  const svg = renderToStaticMarkup(
    React.createElement(ICONS[name], { color: `#${color}`, size: 256, strokeWidth: 1.6 }),
  );
  return sharp(Buffer.from(svg)).resize(256, 256).png().toBuffer();
}

const cache = new Map();
export async function iconData(name, color = "FFB53C") {
  const key = `${name}-${color}`;
  if (!cache.has(key)) {
    const buf = await icon(name, color);
    cache.set(key, "image/png;base64," + buf.toString("base64"));
  }
  return cache.get(key);
}

export async function buildAssets() {
  fs.mkdirSync(OUT, { recursive: true });
  const files = {};

  for (const [name, spec] of Object.entries(BACKGROUNDS)) {
    const file = path.join(OUT, `bg-${name}.jpg`);
    await sharp(Buffer.from(backgroundSvg(spec))).jpeg({ quality: 90 }).toFile(file);
    files[`bg-${name}`] = file;
  }
  for (const [name, color] of [["amber", "#ffb53c"], ["blue", "#4da3ff"], ["red", "#ff5b54"], ["green", "#3ddc84"]]) {
    const file = path.join(OUT, `glow-${name}.png`);
    await sharp(Buffer.from(glowSvg(color))).png().toFile(file);
    files[`glow-${name}`] = file;
  }

  files.logo = path.join(OUT, "logo.png");
  await sharp(Buffer.from(logoSvg())).png().toFile(files.logo);

  // The live view: the render alone (left), and the honeycomb + stats without the
  // development address that sits next to the QR code.
  const live = path.join(SHOTS, "live.jpg");
  files.liveRender = path.join(OUT, "live-render.jpg");
  await sharp(live).extract({ left: 10, top: 12, width: 1905, height: 1072 }).jpeg({ quality: 92 }).toFile(files.liveRender);
  files.liveStats = path.join(OUT, "live-stats.jpg");
  await sharp(live).extract({ left: 0, top: 1140, width: 2750, height: 250 }).jpeg({ quality: 92 }).toFile(files.liveStats);
  files.liveHive = path.join(OUT, "live-hive.jpg");
  await sharp(live).extract({ left: 1950, top: 330, width: 790, height: 790 }).jpeg({ quality: 92 }).toFile(files.liveHive);

  files.phone = path.join(OUT, "phone.png");
  await sharp(path.join(SHOTS, "phone.png")).png().toFile(files.phone);
  files.timetable = path.join(OUT, "timetable.png");
  await sharp(path.join(SHOTS, "timetable.png")).extract({ left: 0, top: 0, width: 1654, height: 1572 }).png().toFile(files.timetable);

  // A field of 1 000 small cells: the thousand simulated devices of the load test.
  files.hexfield = path.join(OUT, "hexfield.png");
  await sharp(Buffer.from(hexfieldSvg())).png().toFile(files.hexfield);

  files.qr = path.join(OUT, "qr.png");
  await QRCode.toFile(files.qr, "https://swarmrelay.duckdns.org", {
    errorCorrectionLevel: "M",
    margin: 1,
    width: 900,
    color: { dark: "#0a0a0c", light: "#ffffff" },
  });

  return files;
}

// Sizes of the source screenshots, for keeping aspect ratios.
export const SIZES = {
  liveRender: [1905, 1072],
  liveStats: [2750, 250],
  liveHive: [790, 790],
  phone: [1170, 1540],
  timetable: [1654, 1572],
  hexfield: [1140, 533],
};
