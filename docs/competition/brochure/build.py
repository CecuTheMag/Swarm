"""Builds outside.html and inside.html of the Рояк brochure (A4 landscape, tri-fold)."""
from glass import *

QR = open("img/qr-worker.svg", encoding="utf-8").read().replace("<svg ", '<svg class="qr" ', 1)

def light(cls, cx, cy, d):
    return f'<div class="light {cls}" style="left:{cx-d/2}mm;top:{cy-d/2}mm;width:{d}mm;height:{d}mm"></div>'

def head(title):
    return f'<!doctype html><html lang="bg"><head><meta charset="utf-8"><title>{title}</title><link rel="stylesheet" href="brochure.css"></head><body>'

# ---------------------------------------------------------------- OUTSIDE (flap | back | cover)
W, H, big = mosaic(pre="cov")
Ws, Hs, small2_svg = mosaic(R=4.6, gap=0.35, radius=2, drop=(), seed=9, pre="bk")
small2 = small2_svg.replace('class="mosaic"', 'class="mosaic" style="display:block;width:40mm;height:auto"')
cover = f"""
<div class="in" style="left:204mm;width:85mm;align-items:stretch">
  <div style="margin-top:6mm">{big.replace('class="mosaic"', 'class="mosaic" style="display:block;width:85mm;height:auto"')}</div>
  <h1 style="margin-top:11mm;margin-left:-1mm">Рояк</h1>
  <p class="lead" style="margin-top:5mm;max-width:76mm">Суперкомпютър от телефоните и компютрите в залата. Отваряш страница – и си част от него.</p>
  <div style="margin-top:auto">
    <p style="margin-bottom:3.4mm;font-weight:600;color:var(--ink);letter-spacing:0.01em">swarmrelay.duckdns.org</p>
    <div class="rule" style="margin-bottom:2.6mm;height:0.5mm;background:#f5a524"></div>
    <p class="small">Национален есенен турнир по информационни технологии „Джон Атанасов“ · направление Разпределени приложения</p>
  </div>
</div>
"""
back = f"""
<div class="in" style="left:103mm;width:85mm">
  <div style="margin-top:2mm">{logo(15)}</div>
  <h2 style="margin-top:4mm">Рояк</h2>
  <p class="lead" style="margin-top:3mm">Доброволен суперкомпютър от браузъри. Свободен софтуер с лиценз GPL-3.0.</p>
  <div class="glass" style="margin-top:10mm;padding:5.4mm 5.8mm">
    <h3 style="font-weight:600;font-size:12pt">Страцимир Стефанов Андонов</h3>
    <p style="margin-top:1.8mm">11 б клас, ППМГ „Гео Милев“, Стара Загора</p>
    <div class="rule" style="margin:3.8mm 0"></div>
    <p class="label">Ръководител</p>
    <p style="margin-top:.8mm"><strong>Светла Антонова Парапанова</strong></p>
    <p class="small" style="margin-top:.5mm">старши учител по информатика и информационни технологии</p>
  </div>
  <div style="margin-top:7mm;display:grid;gap:1.6mm">
    <p class="label">Контакт</p>
    <p><strong>magcecu@gmail.com</strong></p>
  </div>
  <div style="margin-top:auto;margin-bottom:6mm">{small2}</div>
  <div><div class="rule" style="margin-bottom:2.6mm"></div>
  <p class="small">Числата са от измервания в документацията на проекта; 75 % е оценка по разписанието на кабинета.</p></div>
</div>
"""
flap = f"""
<div class="in" style="left:8mm;width:79mm">
  <h2 style="margin-top:2mm">Включи се</h2>
  <p class="lead" style="margin-top:3.5mm">Един допир – и телефонът или компютърът ти става клетка на роя.</p>
  <div class="glass qr-card" style="margin:8mm auto 0">{QR}</div>
  <p class="small" style="text-align:center;margin-top:2.4mm">Насочи камерата към кода</p>
  <div class="glass" style="margin-top:8mm;padding:4.4mm 4.6mm;display:grid;gap:4.4mm">
    <div class="step">{node(1)}<div><h3>Сканирай</h3><p>Отваря се страницата на роя – без инсталиране.</p></div></div>
    <div class="rule"></div>
    <div class="step">{node(2)}<div><h3>Натисни „Включи се“</h3><p>Спираш с един бутон, когато поискаш.</p></div></div>
    <div class="rule"></div>
    <div class="step">{node(3)}<div><h3>Виж клетката си</h3><p>Появява се в живия изглед на роя.</p></div></div>
  </div>
  <p class="small" style="margin-top:auto">Безопасно за доброволеца: 75 % мощност по подразбиране; спира при скрита страница, а в Chrome и Edge – и при батерия под 30 %.</p>
</div>
"""
outside = head("Рояк – диплянка, външна страна") + f"""<div class="page">
{light('silver', 248, 66, 120)}{light('warm', 262, 78, 56)}
{light('silver', 146, 120, 100)}
{light('silver', 48, 150, 96)}
<div class="edge"></div>{flap}{back}{cover}</div></body></html>"""

# ---------------------------------------------------------------- INSIDE (idea | how it works | measured)
Wm, Hm, small = mosaic(R=5.4, gap=0.45, radius=2, drop=(), seed=5, pre="in")
idea = f"""
<div class="in" style="left:8mm;width:85mm">
  <h2 style="margin-top:2mm;max-width:70mm">Всеки екран е клетка</h2>
  <p class="lead" style="margin-top:4mm">Кабинетите стоят празни по-голямата част от седмицата – в нашия около три четвърти от времето по разписанието му (оценка). Всеки ученик носи графичен процесор в джоба си.</p>
  <p style="margin-top:3.5mm">Рояк ги събира в един общ изчислителен ресурс. Нужен е само браузър.</p>
  <div style="margin-top:7mm;display:grid;gap:2.4mm;justify-items:start">
    <div class="glass tag">{mini_cell()}Без инсталиране</div>
    <div class="glass tag">{mini_cell()}Без права на администратор</div>
    <div class="glass tag">{mini_cell()}Графичен процесор през браузъра</div>
  </div>
  <div class="glass" style="margin-top:7mm;padding:4mm 4.6mm"><p>BOINC и Folding@home искат инсталиране и права на администратор – в училище това е невъзможно. <strong>Рояк – не.</strong></p></div>
  <div style="margin-top:auto;margin-left:6mm">{small.replace('class="mosaic"','class="mosaic" style="display:block;width:48mm;height:auto"')}</div>
</div>
"""
how = f"""
<div class="in" style="left:109mm;width:85mm">
  <h2 style="margin-top:2mm;max-width:76mm">От един QR код до готов резултат</h2>
  <div style="margin-top:9mm;display:grid;gap:11.5mm;position:relative">
    <div style="position:absolute;left:6.4mm;top:12mm;bottom:12mm;width:.3mm;background:linear-gradient(180deg,rgba(100,110,128,0),rgba(100,110,128,.8) 12%,rgba(100,110,128,.8) 88%,rgba(100,110,128,0))"></div>
    <div class="step">{node(1)}<div><h3>Свързване</h3><p>Телефони и компютри влизат през един защитен адрес. Допуска се само известен код.</p></div></div>
    <div class="step">{node(2)}<div><h3>Разделяне</h3><p>Голямата задача се реже на малки части; всяка се дава под наем със срок.</p></div></div>
    <div class="step">{node(3)}<div><h3>Проверка</h3><p>Всяка част смятат две устройства. Контролни части с известен отговор хващат измамниците.</p></div></div>
    <div class="step">{node(4)}<div><h3>Събиране</h3><p>Резултатите се сглобяват: картина, седмично разписание, обучена мрежа.</p></div></div>
  </div>
  <div class="glass" style="margin-top:auto;padding:4mm 4.6mm"><p><strong>Измамник?</strong> Хванат за 3,4 s, без нито една приета част (тест със симулирани устройства). Ако е успял да вкара резултати, те се смятат наново.</p></div>
</div>
"""
proof = f"""
<div class="in" style="left:210mm;width:79mm">
  <h2 style="margin-top:2mm">Измерено, не обещано</h2>
  <ul class="facts" style="margin-top:5.5mm">
    <li class="glass">{mini_cell()}<p><strong>13 стари кабинетни компютъра</strong> без WebGPU нарисуваха картина 1920×1080 за около 70 s.</p></li>
    <li class="glass">{mini_cell()}<p><strong>1000 симулирани устройства:</strong> около 1000 резултата в секунда, нито едно отпаднало.</p></li>
    <li class="glass">{mini_cell()}<p><strong>Телефон и лаптоп</strong> се съгласиха в 30 от 30 отговора на езиков модел (9 – дума по дума).</p></li>
  </ul>
  <h3 style="margin-top:5.5mm">Какво смята Рояк</h3>
  <div class="chips" style="margin-top:2.6mm;margin-bottom:4mm">
    <span class="glass tag">Разписание на училище</span><span class="glass tag">Невронни мрежи</span><span class="glass tag">Изображения</span><span class="glass tag">Езикови модели</span><span class="glass tag">Свои програми</span>
  </div>
  <div style="margin-top:auto">
    <div class="glass lens"><img src="img/render-soft-wide.jpg" alt="Картина, изчислена от устройствата на роя"><span class="glass tag over">{mini_cell('live')}Плочка по плочка</span></div>
    <p class="small" style="margin-top:2mm">Готова картина, изчислена от различни устройства.</p>
  </div>
</div>
"""
inside = head("Рояк – диплянка, вътрешна страна") + f"""<div class="page">
{light('silver', 52, 150, 100)}
{light('silver', 152, 176, 96)}
{light('warm', 250, 172, 80)}{light('silver', 252, 100, 100)}
<div class="edge"></div>{idea}{how}{proof}</div></body></html>"""

open("outside.html", "w", encoding="utf-8").write(outside)
open("inside.html", "w", encoding="utf-8").write(inside)
print("built")
