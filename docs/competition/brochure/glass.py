"""The glass vocabulary of Рояк print and brand pieces: hex cells, the cell mosaic, step nodes, the mark."""
import math, random, re

S3 = math.sqrt(3)

def hex_pts(cx, cy, r):
    return [(cx + r * math.cos(math.radians(60 * k + 30)), cy + r * math.sin(math.radians(60 * k + 30))) for k in range(6)]

def hex_path(cx, cy, r):
    p = hex_pts(cx, cy, r)
    return "M" + " L".join(f"{x:.3f},{y:.3f}" for x, y in p) + " Z"

DEFS = """
<defs>
  <linearGradient id="sheen" x1="0" y1="0" x2="1" y2="1">
    <stop offset="0" stop-color="#fff" stop-opacity=".85"/><stop offset=".42" stop-color="#fff" stop-opacity=".10"/>
    <stop offset="1" stop-color="#b9c4d6" stop-opacity=".26"/>
  </linearGradient>
  <linearGradient id="sheenLive" x1="0" y1="0" x2="1" y2="1">
    <stop offset="0" stop-color="#fff" stop-opacity=".95"/><stop offset=".55" stop-color="#ffe3a6" stop-opacity=".85"/>
    <stop offset="1" stop-color="#ffc766" stop-opacity=".9"/>
  </linearGradient>
  <filter id="soft" x="-20%" y="-20%" width="140%" height="150%">
    <feDropShadow dx="0" dy="0.45" stdDeviation="0.7" flood-color="#2b2f3c" flood-opacity=".20"/>
  </filter>
  <filter id="blurSoft" x="-10%" y="-10%" width="120%" height="120%"><feGaussianBlur stdDeviation="2.4"/></filter>
</defs>
"""

def cell_svg(i, cx, cy, r, state, W, img_y, img_size, pre=""):
    d = hex_path(cx, cy, r)
    out = [f'<clipPath id="{pre}c{i}"><path d="{d}"/></clipPath>']
    out.append(f'<g filter="url(#soft)"><path d="{d}" fill="#fff"/></g>')
    if state == "filled":
        out.append(f'<g clip-path="url(#{pre}c{i})"><image href="img/render-soft.jpg" x="0" y="{img_y:.2f}" width="{img_size}" height="{img_size}" preserveAspectRatio="xMidYMid slice"/></g>')
    elif state == "frosted":
        out.append(f'<g clip-path="url(#{pre}c{i})"><image href="img/render-soft.jpg" x="0" y="{img_y:.2f}" width="{img_size}" height="{img_size}" preserveAspectRatio="xMidYMid slice" filter="url(#blurSoft)"/><path d="{d}" fill="#fff" fill-opacity=".86"/></g>')
    else:  # live: a device is computing this cell right now
        out.append(f'<path d="{d}" fill="url(#sheenLive)"/>')
        out.append(f'<path d="{hex_path(cx, cy, r*0.62)}" fill="none" stroke="#fff" stroke-opacity=".9" stroke-width=".45"/>')
        out.append(f'<path d="{hex_path(cx, cy, r*0.34)}" fill="#fff" fill-opacity=".85"/>')
    if state != "live":
        out.append(f'<path d="{d}" fill="url(#sheen)"/>')
    # rim: amber hairline outside, white specular rim on the upper-left edges
    out.append(f'<path d="{d}" fill="none" stroke="#6f7a8c" stroke-opacity=".75" stroke-width=".28"/>')
    p = hex_pts(cx, cy, r * 0.9)
    rim = " ".join(f"{x:.3f},{y:.3f}" for x, y in (p[2], p[3], p[4], p[5]))
    out.append(f'<polyline points="{rim}" fill="none" stroke="#fff" stroke-opacity=".95" stroke-width=".55" stroke-linecap="round" stroke-linejoin="round"/>')
    p2 = hex_pts(cx, cy, r * 0.88)
    rim2 = " ".join(f"{x:.3f},{y:.3f}" for x, y in (p2[5], p2[0], p2[1]))
    out.append(f'<polyline points="{rim2}" fill="none" stroke="#c3cad6" stroke-opacity=".38" stroke-width=".4" stroke-linecap="round" stroke-linejoin="round"/>')
    return "\n".join(out)

def mosaic(R=7.0, gap=0.4, radius=3, drop=((3, -3), (-3, 3), (0, 3)), seed=11, only=None, pre="m"):
    cells = []
    for q in range(-radius, radius + 1):
        for r in range(-radius, radius + 1):
            if abs(q + r) <= radius and (q, r) not in drop:
                cells.append((q, r))
    if only is not None:
        cells = [c for c in cells if only(c)]
    pos = {c: (R * S3 * (c[0] + c[1] / 2), R * 1.5 * c[1]) for c in cells}
    minx = min(x for x, _ in pos.values()) - R * S3 / 2
    maxx = max(x for x, _ in pos.values()) + R * S3 / 2
    miny = min(y for _, y in pos.values()) - R
    maxy = max(y for _, y in pos.values()) + R
    W, H = maxx - minx, maxy - miny
    rng = random.Random(seed)
    items = []
    scored = []
    for c in cells:
        x, y = pos[c]
        t = ((x - minx) / W + (y - miny) / H) / 2 + rng.uniform(-0.17, 0.17)
        scored.append((t, c))
    live = {c for t, c in scored if 0.40 <= t <= 0.47}
    live = set(sorted(live)[:3])
    for i, (t, c) in enumerate(scored):
        x, y = pos[c]
        st = "live" if c in live else ("filled" if t > 0.47 else "frosted")
        items.append(cell_svg(i, x - minx, y - miny, R - gap, st, W, -(W - H) / 2 - 0.0, W, pre))
    return W, H, f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W:.2f} {H:.2f}" width="{W:.2f}mm" height="{H:.2f}mm" class="mosaic">{DEFS}{"".join(items)}</svg>'

def mini_cell(fill="frosted"):
    """a single glass cell used as a bullet"""
    d = hex_path(10, 11, 8.6)
    g = "url(#sheenLive)"
    return (f'<svg class="cell" viewBox="0 0 20 22" xmlns="http://www.w3.org/2000/svg">{DEFS}'
            f'<g filter="url(#soft)"><path d="{d}" fill="{g}"/></g><path d="{d}" fill="url(#sheen)"/>'
            f'<path d="{d}" fill="none" stroke="#8d96a6" stroke-opacity=".6" stroke-width=".5"/>'
            f'<polyline points="{" ".join(f"{x:.2f},{y:.2f}" for x, y in list(hex_pts(10,11,7.8))[2:6])}" fill="none" stroke="#fff" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>')

def node(n):
    d = hex_path(10, 11, 8.7)
    return (f'<svg class="node" viewBox="0 0 20 22" xmlns="http://www.w3.org/2000/svg">{DEFS}'
            f'<g filter="url(#soft)"><path d="{d}" fill="#fff"/></g><path d="{d}" fill="url(#sheen)"/>'
            f'<path d="{d}" fill="none" stroke="#6f7a8c" stroke-opacity=".9" stroke-width=".35"/>'
            f'<polyline points="{" ".join(f"{x:.2f},{y:.2f}" for x, y in list(hex_pts(10,11,7.9))[2:6])}" fill="none" stroke="#fff" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"/>'
            f'<text x="10" y="14.2" text-anchor="middle" font-family="Manrope, sans-serif" font-weight="500" font-size="8.6" fill="#1c1b22">{n}</text></svg>')

def logo(size=14):
    return (f'<svg viewBox="0 0 64 64" width="{size}mm" height="{size}mm" xmlns="http://www.w3.org/2000/svg">'
            '<path d="M32 6l11 6.4v12.7L32 31.5l-11-6.4V12.4z" fill="#f5a524"/>'
            '<path d="M18.7 30.5l11 6.4v12.7l-11 6.4-11-6.4V36.9z" fill="#f5a524" fill-opacity=".78"/>'
            '<path d="M45.3 30.5l11 6.4v12.7l-11 6.4-11-6.4V36.9z" fill="#f5a524" fill-opacity=".55"/></svg>')

