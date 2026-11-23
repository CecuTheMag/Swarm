// Adds what pptxgenjs cannot write: Morph transitions between slides and entrance
// animations inside them. Works on the packed .pptx with JSZip and plain strings.
//
// A slide's plan:
//   { transition: { type: "morph" | "fade", dur: ms },
//     groups: [ { trigger: "auto" | "click", items: [ { name, fx, delay, dur } ] } ] }
// "auto" plays right after the slide appears (or after the previous group), "click"
// waits for the presenter. Items in a group start together, each after its own delay.
import JSZip from "jszip";

const NS_P14 = "http://schemas.microsoft.com/office/powerpoint/2010/main";
const NS_P159 = "http://schemas.microsoft.com/office/powerpoint/2015/09/main";
const NS_MC = "http://schemas.openxmlformats.org/markup-compatibility/2006";

function transitionXml(t) {
  if (!t) return "";
  const dur = t.dur ?? 1200;
  if (t.type === "morph") {
    return (
      `<mc:AlternateContent xmlns:mc="${NS_MC}">` +
      `<mc:Choice xmlns:p159="${NS_P159}" xmlns:p14="${NS_P14}" Requires="p159">` +
      `<p:transition spd="slow" p14:dur="${dur}"><p159:morph option="byObject"/></p:transition>` +
      `</mc:Choice><mc:Fallback><p:transition spd="slow"><p:fade/></p:transition></mc:Fallback>` +
      `</mc:AlternateContent>`
    );
  }
  return (
    `<mc:AlternateContent xmlns:mc="${NS_MC}">` +
    `<mc:Choice xmlns:p14="${NS_P14}" Requires="p14">` +
    `<p:transition spd="slow" p14:dur="${dur}"><p:fade/></p:transition>` +
    `</mc:Choice><mc:Fallback><p:transition spd="slow"><p:fade/></p:transition></mc:Fallback>` +
    `</mc:AlternateContent>`
  );
}

// The behaviours of each effect. `ids` hands out cTn ids; `sp` is the target shape id.
const EFFECTS = {
  fade: { preset: 10, sub: 0, body: (sp, d, id) => setVisible(sp, id) + fadeIn(sp, d, id) },
  float: { preset: 42, sub: 0, body: (sp, d, id) => setVisible(sp, id) + fadeIn(sp, d, id) + move(sp, d, id, "y", "#ppt_y+0.045", "#ppt_y") },
  floatDown: { preset: 42, sub: 0, body: (sp, d, id) => setVisible(sp, id) + fadeIn(sp, d, id) + move(sp, d, id, "y", "#ppt_y-0.045", "#ppt_y") },
  left: { preset: 42, sub: 0, body: (sp, d, id) => setVisible(sp, id) + fadeIn(sp, d, id) + move(sp, d, id, "x", "#ppt_x-0.035", "#ppt_x") },
  right: { preset: 42, sub: 0, body: (sp, d, id) => setVisible(sp, id) + fadeIn(sp, d, id) + move(sp, d, id, "x", "#ppt_x+0.035", "#ppt_x") },
  zoom: { preset: 53, sub: 16, body: (sp, d, id) => setVisible(sp, id) + fadeIn(sp, d, id) + scale(sp, d, id, 0.82) },
  pop: { preset: 53, sub: 16, body: (sp, d, id) => setVisible(sp, id) + fadeIn(sp, Math.round(d * 0.6), id) + scale(sp, d, id, 0.2) },
  wipe: { preset: 22, sub: 8, body: (sp, d, id) => setVisible(sp, id) + filterIn(sp, d, id, "wipe(left)") },
  wipeDown: { preset: 22, sub: 1, body: (sp, d, id) => setVisible(sp, id) + filterIn(sp, d, id, "wipe(up)") },
  wipeUp: { preset: 22, sub: 4, body: (sp, d, id) => setVisible(sp, id) + filterIn(sp, d, id, "wipe(down)") },
};

const tgt = (sp) => `<p:tgtEl><p:spTgt spid="${sp}"/></p:tgtEl>`;
function setVisible(sp, id) {
  return (
    `<p:set><p:cBhvr><p:cTn id="${id()}" dur="1" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn>` +
    `${tgt(sp)}<p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr>` +
    `<p:to><p:strVal val="visible"/></p:to></p:set>`
  );
}
function fadeIn(sp, d, id) {
  return `<p:animEffect transition="in" filter="fade"><p:cBhvr><p:cTn id="${id()}" dur="${d}"/>${tgt(sp)}</p:cBhvr></p:animEffect>`;
}
function filterIn(sp, d, id, filter) {
  return `<p:animEffect transition="in" filter="${filter}"><p:cBhvr><p:cTn id="${id()}" dur="${d}" decel="60000"/>${tgt(sp)}</p:cBhvr></p:animEffect>`;
}
function anim(sp, d, id, attr, from, to) {
  return (
    `<p:anim calcmode="lin" valueType="num"><p:cBhvr additive="base"><p:cTn id="${id()}" dur="${d}" fill="hold" decel="100000"/>` +
    `${tgt(sp)}<p:attrNameLst><p:attrName>${attr}</p:attrName></p:attrNameLst></p:cBhvr>` +
    `<p:tavLst><p:tav tm="0"><p:val><p:strVal val="${from}"/></p:val></p:tav>` +
    `<p:tav tm="100000"><p:val><p:strVal val="${to}"/></p:val></p:tav></p:tavLst></p:anim>`
  );
}
function move(sp, d, id, axis, from, to) {
  return axis === "y"
    ? anim(sp, d, id, "ppt_x", "#ppt_x", "#ppt_x") + anim(sp, d, id, "ppt_y", from, to)
    : anim(sp, d, id, "ppt_x", from, to) + anim(sp, d, id, "ppt_y", "#ppt_y", "#ppt_y");
}
function scale(sp, d, id, k) {
  return anim(sp, d, id, "ppt_w", `#ppt_w*${k}`, "#ppt_w") + anim(sp, d, id, "ppt_h", `#ppt_h*${k}`, "#ppt_h");
}

function timingXml(groups, shapeIds, kinds, slideNo) {
  if (!groups?.length) return "";
  let n = 2;
  const id = () => ++n;
  const builds = new Map();
  let clicks = "";
  groups.forEach((g, gi) => {
    const groupId = id();
    const start =
      g.trigger === "auto"
        ? gi === 0
          ? `<p:cond delay="indefinite"/><p:cond evt="onBegin" delay="0"><p:tn val="2"/></p:cond>`
          : `<p:cond delay="indefinite"/><p:cond evt="onBegin" delay="0"><p:tn val="2"/></p:cond>`
        : `<p:cond delay="indefinite"/>`;
    const innerId = id();
    let effects = "";
    g.items.forEach((it, k) => {
      const sp = shapeIds.get(it.name);
      if (!sp) throw new Error(`slide ${slideNo}: no shape named "${it.name}" to animate`);
      const fx = EFFECTS[it.fx ?? "float"];
      if (!fx) throw new Error(`slide ${slideNo}: unknown effect ${it.fx}`);
      const nodeType = k === 0 ? (g.trigger === "click" ? "clickEffect" : gi === 0 ? "afterEffect" : "afterEffect") : "withEffect";
      const effId = id();
      const body = fx.body(sp, it.dur ?? 700, id);
      effects +=
        `<p:par><p:cTn id="${effId}" presetID="${fx.preset}" presetClass="entr" presetSubtype="${fx.sub}" fill="hold" grpId="0" nodeType="${nodeType}">` +
        `<p:stCondLst><p:cond delay="${it.delay ?? 0}"/></p:stCondLst><p:childTnLst>${body}</p:childTnLst></p:cTn></p:par>`;
      builds.set(sp, kinds.get(it.name));
    });
    clicks +=
      `<p:par><p:cTn id="${groupId}" fill="hold"><p:stCondLst>${start}</p:stCondLst><p:childTnLst>` +
      `<p:par><p:cTn id="${innerId}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>` +
      effects +
      `</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>`;
  });
  // Auto groups after the first one start when the previous group ends: chain them by
  // giving each its own click-less trigger, which PowerPoint plays in sequence.
  const bld = [...builds.entries()]
    .map(([sp, kind]) =>
      kind === "graphicFrame"
        ? `<p:bldGraphic spid="${sp}" grpId="0"><p:bldAsOne/></p:bldGraphic>`
        : kind === "sp"
          ? `<p:bldP spid="${sp}" grpId="0" animBg="1"/>`
          : "",
    )
    .join("");
  return (
    `<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst>` +
    `<p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>` +
    clicks +
    `</p:childTnLst></p:cTn><p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst>` +
    `<p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq>` +
    `</p:childTnLst></p:cTn></p:par></p:tnLst>` +
    (bld ? `<p:bldLst>${bld}</p:bldLst>` : "") +
    `</p:timing>`
  );
}

// Shape ids and element kinds by name, read from the slide XML.
function shapesByName(xml) {
  const ids = new Map();
  const kinds = new Map();
  const re = /<p:(sp|pic|graphicFrame|cxnSp)>\s*<p:nv\w+Pr>\s*<p:cNvPr id="(\d+)" name="([^"]*)"/g;
  let m;
  while ((m = re.exec(xml))) {
    const name = m[3].replace(/&amp;/g, "&");
    if (ids.has(name)) throw new Error(`duplicate shape name "${name}"`);
    ids.set(name, m[2]);
    kinds.set(name, m[1]);
  }
  return { ids, kinds };
}

// The deck's theme colours, so PowerPoint's colour picker offers the brand palette.
function clrScheme(theme) {
  const c = theme.colors;
  const sys = (tag, hex) => `<a:${tag}><a:srgbClr val="${hex}"/></a:${tag}>`;
  return (
    `<a:clrScheme name="${theme.name}">` +
    ["dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"]
      .map((k) => sys(k, c[k]))
      .join("") +
    `</a:clrScheme>`
  );
}

export async function animate(buffer, plans, theme) {
  const zip = await JSZip.loadAsync(buffer);
  if (theme) {
    for (const f of Object.keys(zip.files).filter((f) => /^ppt\/theme\/theme\d+\.xml$/.test(f))) {
      const xml = await zip.file(f).async("string");
      zip.file(f, xml.replace(/<a:clrScheme[\s\S]*?<\/a:clrScheme>/, clrScheme(theme)));
    }
  }
  for (let i = 0; i < plans.length; i++) {
    const file = `ppt/slides/slide${i + 1}.xml`;
    let xml = await zip.file(file).async("string");
    const plan = plans[i] ?? {};
    const { ids, kinds } = shapesByName(xml);
    const extra = transitionXml(plan.transition) + timingXml(plan.groups, ids, kinds, i + 1);
    if (extra) {
      if (!xml.includes("</p:clrMapOvr>")) throw new Error(`${file}: no clrMapOvr`);
      xml = xml.replace(/<p:transition[\s\S]*?<\/p:transition>/, "").replace("</p:clrMapOvr>", `</p:clrMapOvr>${extra}`);
    }
    zip.file(file, xml);
  }
  // Files only: folder entries in the ZIP break opening the deck from cloud storage.
  const out = new JSZip();
  for (const [name, entry] of Object.entries(zip.files)) {
    if (!entry.dir) out.file(name, await entry.async("uint8array"), { createFolders: false });
  }
  return out.generateAsync({ type: "nodebuffer", compression: "DEFLATE" });
}
