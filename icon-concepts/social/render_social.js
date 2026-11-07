// SPDX-License-Identifier: AGPL-3.0-or-later
// The GitHub social preview (1280 x 640) from the chosen mark, for the owner to upload (lane ICON, decision 0542).
// Text in IBM Plex Sans (OFL-1.1, the project's interface font, apps/openchuck-ui/assets/fonts).
const { chromium } = require('playwright');
const fs = require('fs');
const repo = '/home/user/OpenChuckCAM/';
const font = fs.readFileSync(repo + 'apps/openchuck-ui/assets/fonts/IBMPlexSans-SemiBold.ttf').toString('base64');
const fontR = fs.readFileSync(repo + 'apps/openchuck-ui/assets/fonts/IBMPlexSans-Regular.ttf').toString('base64');
const mark = fs.readFileSync(repo + 'apps/openchuck-ui/assets/icon/mark-white.svg').toString('base64');
const html = `<html><head><style>
@font-face{font-family:P;font-weight:600;src:url(data:font/ttf;base64,${font})}
@font-face{font-family:P;font-weight:400;src:url(data:font/ttf;base64,${fontR})}
body{margin:0;width:1280px;height:640px;background:#1B1F24;display:flex;align-items:center;justify-content:center;gap:64px;font-family:P}
h1{margin:0;color:#F2F4F7;font-weight:600;font-size:96px;letter-spacing:0.5px}
p{margin:12px 0 0;color:#AEB6C1;font-weight:400;font-size:38px}
</style></head><body><img src="data:image/svg+xml;base64,${mark}" width="280" height="280">
<div><h1>OpenChuckCAM</h1><p>CAM for 4-axis rotary machining</p></div></body></html>`;
(async () => {
  const b = await chromium.launch(); const p = await b.newPage({ viewport: { width: 1280, height: 640 }, deviceScaleFactor: 1 });
  await p.setContent(html); await p.evaluate(() => document.fonts.ready);
  await p.screenshot({ path: 'social-preview-1280x640.png' }); await b.close();
})();
