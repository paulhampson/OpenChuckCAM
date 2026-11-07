// SPDX-License-Identifier: AGPL-3.0-or-later
// Mock-ups of the chosen mark on each surface, for the owner's colour question (throwaway, lane ICON).
const { chromium } = require('playwright');
const fs = require('fs');
const src = n => `data:image/svg+xml;base64,${Buffer.from(fs.readFileSync(n + '.svg')).toString('base64')}`;
const im = (n, s, extra = '') => `<img src="${src(n)}" width="${s}" height="${s}" style="display:block;${extra}">`;
const C = [['mark-ink', 'a/b ink'], ['mark-white', 'b white'], ['mark-slate', 'a slate #6E7F96'], ['mark-orange', 'a orange #C46400'], ['tile-ink', 'd white on ink tile'], ['tile-white', 'd ink on white tile']];
const cell = (inner, label, fg) => `<div class="c">${inner}<span style="color:${fg}">${label}</span></div>`;
let h = `<html><head><style>body{margin:0;font:14px sans-serif;background:#fff}h2{margin:0;padding:10px 16px;background:#ddd;font-size:17px}
.row{display:flex}.p{flex:1;display:flex;gap:22px;align-items:flex-end;padding:16px 18px;flex-wrap:wrap}.c{display:flex;flex-direction:column;align-items:center;gap:6px;font-size:11px;text-align:center;max-width:180px}
.bar{display:flex;gap:14px;align-items:center;padding:8px 14px;border-radius:8px}.ph{width:24px;height:24px;border-radius:5px;background:#8a8f99}</style></head><body>`;
// 1. the candidates
h += `<h2>1. The chosen mark (A6-C3) in the candidate colours, and on a tile (128, 32, 16 px)</h2><div class="row">`;
for (const [bg, fg] of [['#F4F4F2', '#222'], ['#161616', '#eee']])
  h += `<div class="p" style="background:${bg}">` + C.map(([n, l]) => cell(`<div style="display:flex;gap:8px;align-items:flex-end">${im(n, 96)}${im(n, 32)}${im(n, 16)}</div>`, l, fg)).join('') + '</div>';
h += '</div>';
// 2. Windows taskbar
h += `<h2>2. Windows taskbar and title bar (24 px icon; light and dark taskbar)</h2><div class="row">`;
for (const [bg, fg, bar] of [['#DCE3EA', '#222', '#F3F3F3'], ['#0E1621', '#eee', '#202020']])
  h += `<div class="p" style="background:${bg}">` + C.map(([n, l]) => cell(`<div class="bar" style="background:${bar}"><div class="ph"></div>${im(n, 24)}<div class="ph"></div></div>`, l, fg)).join('') + '</div>';
h += '</div>';
// 3. Explorer
h += `<h2>3. Windows Explorer, large icons (48 px; light and dark theme)</h2><div class="row">`;
for (const [bg, fg] of [['#FFFFFF', '#222'], ['#191919', '#eee']])
  h += `<div class="p" style="background:${bg}">` + C.map(([n, l]) => cell(`${im(n, 48)}<span style="color:${fg};font-size:12px">openchuck-ui.exe</span>`, l, fg)).join('') + '</div>';
h += '</div>';
// 4. macOS Dock
const jail = n => `<div style="width:64px;height:64px;border-radius:15px;background:#E6E6E6;display:flex;align-items:center;justify-content:center">${im(n, 46)}</div>`;
const D = [['mark-ink', 'tile-less ink, as macOS 26 shows it (grey tile added)', jail], ['mark-white', 'tile-less white, as macOS 26 shows it', jail], ['tile-ink', 'd white on ink tile', n => im(n, 64)], ['tile-white', 'd ink on white tile', n => im(n, 64)]];
h += `<h2>4. macOS Dock (64 px shown; light and dark)</h2><div class="row">`;
for (const [bg, fg, dock] of [['#9DB4CC', '#111', 'rgba(240,240,240,0.75)'], ['#1C2533', '#eee', 'rgba(60,60,60,0.8)']])
  h += `<div class="p" style="background:${bg}"><div class="bar" style="background:${dock};gap:18px;padding:10px 18px;border-radius:20px">` + D.map(([n, l, f]) => cell(f(n), '', fg)).join('') + `</div><div style="font-size:11px;color:${fg};max-width:520px">left to right: ${D.map(d => d[1]).join('; ')}</div></div>`;
h += '</div>';
// 5. browser tabs
const tab = (n, bg, fg) => `<div style="display:flex;gap:8px;align-items:center;background:${bg};color:${fg};padding:8px 12px;border-radius:8px 8px 0 0;width:150px">${im(n, 16)}<span style="font-size:12px">OpenChuckCAM user guide</span></div>`;
h += `<h2>5. User guide favicon in a browser tab (16 px; light and dark browser)</h2><div class="row">`;
for (const [bg, fg, t, adaptive] of [['#DEE1E6', '#222', '#FFFFFF', 'mark-ink'], ['#202124', '#eee', '#35363A', 'mark-white']])
  h += `<div class="p" style="background:${bg}">` + [[adaptive, 'b adaptive SVG (follows the browser)'], ['mark-slate', 'a slate'], ['mark-orange', 'a orange'], ['tile-ink', 'd ink tile']].map(([n, l]) => cell(tab(n, t, fg), l, bg === '#202124' ? '#eee' : '#222')).join('') + '</div>';
h += '</div></body></html>';
(async () => {
  const b = await chromium.launch(); const p = await b.newPage({ deviceScaleFactor: 1 });
  await p.setViewportSize({ width: 1600, height: 400 }); await p.setContent(h);
  await p.screenshot({ path: 'surfaces-sheet.png', fullPage: true }); await b.close();
})();
