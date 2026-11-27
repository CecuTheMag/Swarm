"""Builds the brand images of the README: banner (light, dark), social preview, logo mark.
Usage: python3 build.py && ./render.sh"""
import sys, os
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "competition", "brochure"))
from glass import mosaic, logo  # noqa: E402

BRO = "../competition/brochure"

def mark(px):
    return (f'<svg viewBox="0 0 64 64" width="{px}" height="{px}" xmlns="http://www.w3.org/2000/svg">'
            '<path d="M32 6l11 6.4v12.7L32 31.5l-11-6.4V12.4z" fill="#f5a524"/>'
            '<path d="M18.7 30.5l11 6.4v12.7l-11 6.4-11-6.4V36.9z" fill="#f5a524" fill-opacity=".8"/>'
            '<path d="M45.3 30.5l11 6.4v12.7l-11 6.4-11-6.4V36.9z" fill="#f5a524" fill-opacity=".6"/></svg>')

def page(title, w, h, dark, layout):
    W, H, svg = mosaic(R=7.0, gap=0.4, seed=11, pre="b")
    svg = svg.replace('href="img/', f'href="{BRO}/img/').replace('class="mosaic"', 'class="mosaic" style="display:block;height:100%;width:auto"')
    bg, ink, body, rule = ("#0f1014", "#f3f4f8", "#b8bcc8", "rgba(255,255,255,.22)") if dark else ("#ffffff", "#1c1b22", "#3f3e48", "rgba(96,106,124,.5)")
    glow = "radial-gradient(circle closest-side, rgba(255,196,110,.30), rgba(255,196,110,0) 70%)" if dark else "radial-gradient(circle closest-side, rgba(196,207,224,.55), rgba(196,207,224,0) 72%)"
    tags = ["Без инсталиране", "Графичен процесор в браузъра", "Всяка част се проверява"]
    chip_bg = "rgba(255,255,255,.08)" if dark else "rgba(255,255,255,.85)"
    chip_ring = "rgba(255,255,255,.28)" if dark else "rgba(96,106,124,.55)"
    return f"""<!doctype html><html lang="bg"><head><meta charset="utf-8"><title>{title}</title><style>
@font-face {{ font-family: Manrope; font-weight: 200 800; src: url("{BRO}/fonts/manrope-400-cyrillic.woff2") format("woff2"); unicode-range: U+0301, U+0400-045F, U+0490-0491, U+04B0-04B1, U+2116; }}
@font-face {{ font-family: Manrope; font-weight: 200 800; src: url("{BRO}/fonts/manrope-400-latin.woff2") format("woff2"); unicode-range: U+0000-00FF, U+2000-206F, U+20AC, U+2122, U+2212; }}
* {{ box-sizing: border-box; }} html, body {{ margin: 0; background: {bg}; }}
.b {{ position: relative; width: {w}px; height: {h}px; overflow: hidden; background: {bg}; font-family: Manrope, sans-serif; font-feature-settings: "locl" 0; color: {ink}; }}
.glow {{ position: absolute; right: {layout['gx']}px; top: {layout['gy']}px; width: {layout['gs']}px; height: {layout['gs']}px; background: {glow}; border-radius: 50%; }}
.art {{ position: absolute; right: {layout['ax']}px; top: {layout['ay']}px; height: {layout['ah']}px; }}
.t {{ position: absolute; left: {layout['tx']}px; top: {layout['ty']}px; }}
.mark {{ display: flex; align-items: center; gap: 18px; margin-bottom: {layout['mb']}px; }}
h1 {{ margin: 0; font-weight: 200; font-size: {layout['h1']}px; line-height: .95; letter-spacing: .01em; }}
p {{ margin: {layout['pm']}px 0 0; font-size: {layout['p']}px; line-height: 1.35; font-weight: 350; color: {body}; max-width: {layout['pw']}px; }}
.row {{ display: flex; gap: 12px; margin-top: {layout['rm']}px; flex-wrap: wrap; max-width: {layout['pw']+80}px; }}
.row span {{ font-size: {layout['c']}px; font-weight: 550; color: {ink}; padding: 11px 18px; border-radius: 8px; background: {chip_bg}; box-shadow: 0 0 0 1.5px {chip_ring}, inset 0 1.5px 0 rgba(255,255,255,{'.35' if dark else '1'}); }}
</style></head><body><div class="b">
<div class="glow"></div>
<div class="art">{svg}</div>
<div class="t"><div class="mark">{mark(layout['logo'])}</div><h1>Рояк</h1>
<p>Доброволен суперкомпютър от браузъри: телефоните и компютрите в залата смятат заедно.</p>
<div class="row">{''.join(f'<span>{t}</span>' for t in tags)}</div></div>
</div></body></html>"""

banner = dict(gx=-40, gy=-70, gs=700, ax=84, ay=40, ah=440, tx=96, ty=62, mb=18, h1=132, pm=18, p=26, pw=760, rm=24, c=19, logo=54)
social = dict(gx=-60, gy=-40, gs=760, ax=56, ay=64, ah=512, tx=72, ty=96, mb=22, h1=124, pm=18, p=26, pw=520, rm=24, c=19, logo=54)
open(os.path.join(HERE, "banner-light.html"), "w", encoding="utf-8").write(page("Рояк", 1600, 520, False, banner))
open(os.path.join(HERE, "banner-dark.html"), "w", encoding="utf-8").write(page("Рояк", 1600, 520, True, banner))
open(os.path.join(HERE, "social-preview.html"), "w", encoding="utf-8").write(page("Рояк", 1280, 640, False, social))
open(os.path.join(HERE, "logo.svg"), "w", encoding="utf-8").write(logo(64).replace('width="64mm" height="64mm"', 'width="64" height="64"'))
print("built")
