#!/usr/bin/env node
// PROTOTYPE tool. Measures the game rectangle in each layout in headless Chrome and
// prints the layout table in README.md. Needs playwright-core, like shoot.mjs.
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const { chromium } = await import(process.env.PLAYWRIGHT_CORE ?? 'playwright-core');
const page0 = pathToFileURL(join(dirname(fileURLToPath(import.meta.url)), 'index.html')).href;
const layouts = [['A, side rail', 'variant=A'], ['A, panel hidden', 'variant=A&rail=0'], ['B, bottom dock', 'variant=B'], ['C, drawer open', 'variant=C'], ['C, drawer closed', 'variant=C&drawer=0']];
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const p = await browser.newPage();

async function measure(q, vw, vh) {
  await p.setViewportSize({ width: vw, height: vh });
  await p.goto(`${page0}?${q}&mode=client&state=running&freeze=1&chrome=0`);
  await p.waitForSelector('.game');
  return p.evaluate(() => {
    const box = (s) => document.querySelector(s).getBoundingClientRect();
    const g = box('.game'), st = box('.stage'), w = box('.window');
    const size = (r) => `${Math.round(r.width)} × ${Math.round(r.height)}`;
    return { win: size(w), game: size(g), gw: g.width, gh: g.height, bars: Math.round((1 - (g.width * g.height) / (st.width * st.height)) * 100) };
  });
}
async function smallest(fits, lo, hi) {
  while (hi - lo > 1) { const mid = (lo + hi) >> 1; if (await fits(mid)) hi = mid; else lo = mid; }
  return hi;
}

const big = (await measure('variant=A', 1600, 1000)).win;
const small = (await measure('variant=A', 1280, 800)).win;
console.log(`| Layout | Game in a ${big} window | Black bars in the game pane | Game in a ${small} window | Smallest window for a 960 × 540 game |`);
console.log('| --- | --- | --- | --- | --- |');
for (const [name, q] of layouts) {
  const a = await measure(q, 1600, 1000);
  const b = await measure(q, 1280, 800);
  const vw = await smallest(async (w) => (await measure(q, w, 1600)).gw >= 960, 900, 2400);
  const vh = await smallest(async (h) => (await measure(q, 2400, h)).gh >= 540, 500, 1600);
  const m = await measure(q, vw, vh);
  const reached = m.gw >= 960 && m.gh >= 540 ? '' : ` (game only ${m.game})`;
  console.log(`| ${name} | ${a.game} | ${a.bars}% | ${b.game} | ${m.win}${reached} |`);
}
await browser.close();
