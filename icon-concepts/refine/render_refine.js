// SPDX-License-Identifier: AGPL-3.0-or-later
// Renders the concept A refinement options and one contact sheet (throwaway, lane ICON).
const { chromium } = require('playwright');
const fs = require('fs');
const opts = JSON.parse(fs.readFileSync('options.json', 'utf8'));
const sizes = [512, 128, 32, 16];
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ deviceScaleFactor: 1 });
  fs.mkdirSync('png', { recursive: true });
  const files = [...new Set(opts.flatMap(o => [o.light, o.dark]))];
  for (const n of files) {
    const svg = fs.readFileSync(n + '.svg', 'utf8');
    for (const s of sizes) {
      await p.setViewportSize({ width: s, height: s });
      await p.setContent(`<html><body style="margin:0;background:transparent"><img style="display:block" width="${s}" height="${s}" src="data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}"></body></html>`);
      await p.screenshot({ path: `png/${n}-${s}.png`, omitBackground: true, clip: { x: 0, y: 0, width: s, height: s } });
    }
  }
  const img = f => `data:image/png;base64,${fs.readFileSync(f).toString('base64')}`;
  const panel = (n, bg, fg) => `<div class="panel" style="background:${bg};color:${fg}">` +
    sizes.map(s => `<div class="cell"><img src="${img(`png/${n}-${s}.png`)}" width="${s === 512 ? 256 : s}"><span>${s}${s === 512 ? ' (half)' : ''}</span></div>`).join('') +
    [32, 16].map(s => `<div class="cell"><img src="${img(`png/${n}-${s}.png`)}" width="96" style="image-rendering:pixelated"><span>${s} ×${96 / s}</span></div>`).join('') + '</div>';
  let html = `<html><head><style>body{margin:0;font:15px sans-serif;background:#fff}.row{display:flex;align-items:stretch;border-top:2px solid #888}
  .lbl{width:190px;padding:12px;font-size:14px}.lbl b{font-size:22px;display:block}.panel{display:flex;align-items:center;gap:18px;padding:14px 18px}
  .cell{display:flex;flex-direction:column;align-items:center;gap:4px;font-size:12px}h2{margin:0;padding:12px 16px;background:#ddd}</style></head><body>`;
  let group = '';
  for (const o of opts) {
    if (o.group !== group) { group = o.group; html += `<h2>${group === 'colour' ? 'Colour options (on variation A1); light and dark backgrounds' : 'Variations (in colour C1, A6 also in C3); light and dark backgrounds'}</h2>`; }
    html += `<div class="row"><div class="lbl"><b>${o.id}</b>${o.label}</div>${panel(o.light, '#F4F4F2', '#222')}${panel(o.dark, '#161616', '#EEE')}</div>`;
  }
  html += '</body></html>';
  await p.setViewportSize({ width: 1700, height: 400 });
  await p.setContent(html);
  await p.screenshot({ path: 'contact-sheet-refine.png', fullPage: true });
  await b.close();
})();
