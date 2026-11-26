// Print-safety check: how much of each page carries dark or heavy colour.
const sharp = require(process.argv[2] + "/sharp");
(async () => {
  for (const f of process.argv.slice(3)) {
    const { data, info } = await sharp(f).resize(1200).raw().toBuffer({ resolveWithObject: true });
    const n = info.width * info.height, ch = info.channels;
    let dark = 0, heavy = 0, mid = 0, tint = 0, maxSat = 0;
    for (let i = 0; i < n; i++) {
      const r = data[i * ch], g = data[i * ch + 1], b = data[i * ch + 2];
      const l = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;
      const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
      if (l < 0.35) dark++; else if (l < 0.55) heavy++; else if (l < 0.78) mid++; else if (l < 0.97) tint++;
    }
    const p = (x) => (100 * x / n).toFixed(2) + " %";
    console.log(f.split("/").pop(), "very dark (<35%):", p(dark), "| heavy (35-55%):", p(heavy), "| mid (55-78%):", p(mid), "| light tint (78-97%):", p(tint));
  }
})();
