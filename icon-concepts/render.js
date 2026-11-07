// SPDX-License-Identifier: AGPL-3.0-or-later
// Renders each concept SVG to PNG sizes and builds a contact sheet (throwaway, lane ICON).
const { chromium } = require('playwright');
const fs = require('fs');
const names = fs.readdirSync('.').filter(f => f.endsWith('.svg')).map(f => f.slice(0, -4)).sort();
const sizes = [512, 128, 32, 16];
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ deviceScaleFactor: 1 });
  fs.mkdirSync('png', { recursive: true });
  for (const n of names) {
    const svg = fs.readFileSync(n + '.svg', 'utf8');
    for (const s of sizes) {
      await p.setViewportSize({ width: s, height: s });
      await p.setContent(`<html><body style="margin:0;background:transparent"><img style="display:block" width="${s}" height="${s}" src="data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}"></body></html>`);
      await p.screenshot({ path: `png/${n}-${s}.png`, omitBackground: true, clip: { x: 0, y: 0, width: s, height: s } });
    }
  }
  const img = f => `data:image/png;base64,${fs.readFileSync(f).toString('base64')}`;
  const row = (n, bg, fg) => `<div class="row" style="background:${bg};color:${fg}"><div class="lbl">${n}</div>` +
    sizes.map(s => `<div class="cell"><img src="${img(`png/${n}-${s}.png`)}" width="${s === 512 ? 256 : s}"><span>${s}px${s === 512 ? ' (shown half)' : ''}</span></div>`).join('') +
    [32, 16].map(s => `<div class="cell"><img src="${img(`png/${n}-${s}.png`)}" width="96" style="image-rendering:pixelated"><span>${s}px ×${96 / s}</span></div>`).join('') + '</div>';
  const html = `<html><head><style>body{margin:0;font:14px sans-serif}.row{display:flex;align-items:center;gap:28px;padding:16px 24px}
  .lbl{width:180px;font-weight:bold}.cell{display:flex;flex-direction:column;align-items:center;gap:6px}</style></head><body>` +
    names.map(n => row(n, '#F4F4F2', '#222') + row(n, '#1A1A1A', '#EEE')).join('') + '</body></html>';
  await p.setViewportSize({ width: 1100, height: 400 });
  await p.setContent(html);
  await p.screenshot({ path: 'contact-sheet.png', fullPage: true });
  await b.close();
})();
