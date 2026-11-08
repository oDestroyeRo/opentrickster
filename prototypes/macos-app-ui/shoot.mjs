#!/usr/bin/env node
// PROTOTYPE tool. Screenshots the prototype states and drives its interactions in
// headless Chrome. Needs playwright-core: set PLAYWRIGHT_CORE to its index.mjs, or
// install it next to this file. Exits 1 on any page error or failed check.
import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const { chromium } = await import(process.env.PLAYWRIGHT_CORE ?? 'playwright-core');
const here = dirname(fileURLToPath(import.meta.url));
const page0 = pathToFileURL(join(here, 'index.html')).href;
const out = join(here, 'screenshots');
mkdirSync(out, { recursive: true });

const shots = [
  ['a-client-running', 'variant=A&mode=client&state=running'],
  ['a-client-running-game-first', 'variant=A&mode=client&state=running&rail=0'],
  ['a-clientless-running-game-first', 'variant=A&mode=clientless&state=running&rail=0'],
  ['a-client-paused-potions-game-first', 'variant=A&mode=client&state=paused-potions&rail=0'],
  ['a-client-ready-blocked-game-first', 'variant=A&mode=client&state=ready-blocked&rail=0'],
  ['a-client-running-game-first-light', 'variant=A&mode=client&state=running&rail=0&theme=light'],
  ['b-client-running', 'variant=B&mode=client&state=running'],
  ['c-client-running', 'variant=C&mode=client&state=running'],
  ['c-client-running-drawer-closed', 'variant=C&mode=client&state=running&drawer=0'],
  ['a-clientless-running', 'variant=A&mode=clientless&state=running'],
  ['b-clientless-running', 'variant=B&mode=clientless&state=running'],
  ['c-clientless-running', 'variant=C&mode=clientless&state=running&drawer=0'],
  ['a-client-running-light', 'variant=A&mode=client&state=running&theme=light'],
  ['b-clientless-running-light', 'variant=B&mode=clientless&state=running&theme=light'],
  ['c-client-paused-potions-light', 'variant=C&mode=client&state=paused-potions&theme=light'],
  ['a-client-loading', 'variant=A&mode=client&state=loading'],
  ['a-client-login', 'variant=A&mode=client&state=login'],
  ['a-client-ready', 'variant=A&mode=client&state=ready'],
  ['a-client-paused-potions', 'variant=A&mode=client&state=paused-potions'],
  ['a-client-paused-goal-digging', 'variant=A&mode=client&state=paused-goal&activity=digging'],
  ['a-client-reconnecting', 'variant=A&mode=client&state=reconnecting'],
  ['a-client-error-build', 'variant=A&mode=client&state=error-build'],
  ['a-client-error-webkit', 'variant=A&mode=client&state=error-client'],
  ['a-clientless-connect', 'variant=A&mode=clientless&state=connect'],
  ['a-clientless-auth-failed', 'variant=A&mode=clientless&state=auth-failed'],
  ['a-clientless-connecting', 'variant=A&mode=clientless&state=connecting'],
  ['a-clientless-disconnected', 'variant=A&mode=clientless&state=disconnected'],
  ['a-client-setup-combat', 'variant=A&mode=client&state=ready&tab=setup'],
  ['b-client-setup-digging', 'variant=B&mode=client&state=ready&tab=setup&activity=digging'],
  ['b-client-digging-running', 'variant=B&mode=client&state=running&activity=digging'],
  ['a-client-switch-sheet', 'variant=A&mode=client&state=switch-sheet'],
  ['a-client-open-questions', 'variant=A&mode=client&state=running&notes=1'],
];

const results = [];
const check = (ok, what) => { results.push({ ok, what }); console.log(`${ok ? 'pass' : 'FAIL'}  ${what}`); };

const browser = await chromium.launch({ channel: 'chrome', headless: true });
const ctx = await browser.newContext({ viewport: { width: 1600, height: 1000 }, deviceScaleFactor: 1, colorScheme: 'dark' });
const errors = [];
ctx.on('page', (p) => {
  p.on('pageerror', (e) => errors.push(`${p.url()} pageerror ${e.message}`));
  p.on('console', (m) => { if (m.type() === 'error') errors.push(`${p.url()} console ${m.text()}`); });
});

async function open(query) {
  const p = await ctx.newPage();
  await p.goto(`${page0}?${query}`);
  await p.waitForSelector('.window');
  await p.evaluate(() => document.fonts.ready);
  return p;
}

async function layoutChecks(p, name) {
  const r = await p.evaluate(() => {
    const box = (el) => el && el.getBoundingClientRect();
    const game = box(document.querySelector('.game'));
    const overflow = [...document.querySelectorAll('.toolbar,.rail,.dock,.drawer,.ticker,.strip,.foot,.tabs')]
      .filter((el) => el.scrollWidth > el.clientWidth + 1).map((el) => el.className);
    const floating = game ? [...document.querySelectorAll('.banner,.tip,.flash')].map(box)
      .filter((b) => b && b.width && b.left < game.right && b.right > game.left && b.top < game.bottom && b.bottom > game.top) : [];
    const clipped = [...document.querySelectorAll('.dock-cols>.section')]
      .filter((el) => el.scrollHeight > el.clientHeight + 1).map((el) => `${el.querySelector('.sec-title')?.textContent ?? el.className} by ${el.scrollHeight - el.clientHeight}px`);
    const tight = [...document.querySelectorAll('.window *')]
      .filter((el) => !el.closest('.game-splash,.scene,.note,select') && el.getClientRects().length && [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim()))
      .filter((el) => { const cs = getComputedStyle(el); return cs.lineHeight === 'normal' || parseFloat(cs.lineHeight) < 1.5 * parseFloat(cs.fontSize) - 0.01; })
      .map((el) => `${el.tagName.toLowerCase()}.${[...el.classList].join('.')}`);
    const fresh = [...document.querySelectorAll('.chip')].filter((el) => /^(Live, |No live state|No character yet|State \d+ s old)/.test(el.textContent.trim())).length;
    const freshWanted = document.querySelector('.ticker[data-banner]') ? 0 : 1;
    return { game: game && { w: game.width, h: game.height }, overflow, floatingOverGame: floating.length, clipped, fresh, freshWanted, tight: [...new Set(tight)] };
  });
  if (r.game) check(Math.abs(r.game.w / r.game.h - 16 / 9) < 0.01, `${name}: game rectangle is 16:9 (${Math.round(r.game.w)}x${Math.round(r.game.h)})`);
  check(r.overflow.length === 0, `${name}: no horizontal overflow in chrome regions ${r.overflow.join(' ')}`);
  check(r.floatingOverGame === 0, `${name}: no GPUI-drawn banner or tooltip overlaps the game rectangle`);
  check(r.clipped.length === 0, `${name}: dock overview columns show their content without scrolling ${r.clipped.join(', ')}`);
  check(r.tight.length === 0, `${name}: every text line is at least 1.5 times its font size, room for Thai marks ${r.tight.join(' ')}`);
  check(r.fresh === r.freshWanted, `${name}: state freshness shows ${r.freshWanted === 1 ? 'exactly once' : 'nowhere while the ticker holds a banner'} (found ${r.fresh})`);
}

let taken = 0;
const shoot = (p, name) => p.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))).then(() => p.screenshot({ path: join(out, `${name}.png`), animations: 'disabled' })).then(() => taken++);
for (const [name, q] of shots) {
  const p = await open(`${q}&freeze=1&chrome=0`);
  await layoutChecks(p, name);
  await shoot(p, name);
  await p.close();
}

const desk = 28;
for (const [win, vw, vh] of [['1252x772', 1280, 800], ['972x641', 972 + desk, 641 + desk]]) {
  for (const mode of ['client', 'clientless']) {
    for (const state of ['pausing', 'running', 'ready-blocked', 'paused-potions', 'paused-reconnected', 'reconnecting', 'error-build']) {
      for (const rail of ['0', '1']) {
        const p = await open(`variant=A&mode=${mode}&state=${state}&rail=${rail}&freeze=1&chrome=0`);
        await p.setViewportSize({ width: vw, height: vh });
        await layoutChecks(p, `${win} window, ${mode} ${state}, panel ${rail === '0' ? 'hidden' : 'shown'}`);
        if (state === 'running' && (win === '972x641' || rail === '0')) await shoot(p, `a-${mode}-running-${rail === '0' ? 'game-first-' : ''}${win}`);
        await p.close();
      }
    }
  }
}
for (const [w, shown] of [[1232, true], [1231, false]]) {
  const p = await open('variant=A&mode=client&state=paused-reconnected&rail=0&freeze=1&chrome=0');
  await p.setViewportSize({ width: w + desk, height: 800 });
  const r = await p.evaluate(() => ({ minis: !!document.querySelector('.minis')?.getClientRects().length, over: document.querySelector('.toolbar').scrollWidth - document.querySelector('.toolbar').clientWidth, win: document.querySelector('.window').getBoundingClientRect().width }));
  check(r.win === w && r.minis === shown && r.over <= 0, `in a ${r.win} px window with the widest status, the mini meters are ${shown ? 'shown' : 'hidden'} and the toolbar fits`);
  await p.close();
}

const pill = (p) => p.locator('.pill').first().innerText();
const waitPill = (p, text) => p.waitForFunction((t) => document.querySelector('.pill')?.innerText.includes(t), text, { timeout: 4000 }).then(() => true, () => false);

{
  const p = await open('variant=A&mode=client&state=running');
  await p.keyboard.press('Meta+P');
  check(await waitPill(p, 'Pausing'), 'Cmd+P moves running to pausing');
  check(await waitPill(p, 'Paused'), 'pausing settles to paused');
  await p.keyboard.press('Meta+R');
  check(await waitPill(p, 'Running'), 'Cmd+R resumes the run');
  await p.locator('.seg[aria-label="Operating mode"] button', { hasText: 'Clientless' }).click();
  check(await p.locator('.flash').innerText().then((t) => t.includes('Stop the run first')), 'mode switch is refused during a run');
  await p.keyboard.press('Meta+.');
  check(await waitPill(p, 'Ready'), 'Cmd+. stops the run and returns to ready');
  await p.locator('.seg[aria-label="Operating mode"] button', { hasText: 'Clientless' }).click();
  check(await p.locator('.sheet').isVisible(), 'mode switch after stop asks with a native-style sheet');
  await p.locator('.sheet .btn.primary').click();
  check(await waitPill(p, 'Not connected'), 'switching to clientless lands on the sign-in screen');
  await p.close();
}
{
  const p = await open('variant=A&mode=client&state=running');
  await p.keyboard.press('Meta+P');
  await p.keyboard.press('Meta+.');
  check(await waitPill(p, 'Stopping'), 'Cmd+. while pausing stops the run');
  check(await waitPill(p, 'Ready'), 'a stop from pausing settles to ready');
  await p.close();
}
{
  const p = await open('variant=B&mode=client&state=running');
  await p.locator('.pbar select').selectOption('Resource limit and recovery');
  await p.locator('.pbar button', { hasText: 'Step 1' }).click();
  check(await waitPill(p, 'Out of HP potions'), 'potions running out pauses with the reason in the status');
  check(await p.locator('.controls .btn.primary', { hasText: 'Resume' }).isDisabled(), 'Resume stays locked while potions are 0');
  check(await p.locator('.banner').innerText().then((t) => t.includes('Resume unlocks when the bag has HP potions again')), 'the banner says what unlocks Resume');
  await p.locator('.pbar button', { hasText: 'Step 2' }).click();
  check((await pill(p)).includes('Paused'), 'trying to resume with 0 potions keeps the run paused');
  await p.locator('.pbar button', { hasText: 'Step 3' }).click();
  check(await p.locator('.controls .btn.primary', { hasText: 'Resume' }).isEnabled(), 'restocking in the game unlocks Resume');
  await p.locator('.pbar button', { hasText: 'Step 4' }).click();
  check(await waitPill(p, 'Running'), 'resume after restock returns to running');
  await p.close();
}
{
  const p = await open('variant=C&mode=clientless&state=running');
  await p.locator('.pbar button', { hasText: 'Simulate' }).click();
  await p.locator('.simmenu button', { hasText: 'Connection drops' }).click();
  check(await waitPill(p, 'Reconnecting'), 'a dropped connection shows reconnecting');
  check(await waitPill(p, 'Check after reconnect'), 'a resumed session pauses for a check instead of resuming actions');
  await p.locator('.window').click({ position: { x: 5, y: 300 } });
  await p.keyboard.press('ArrowLeft');
  check(await p.locator('.pbar .v').innerText().then((t) => t.startsWith('B')), 'arrow keys cycle layout variants');
  await p.close();
}
{
  const p = await open('variant=A&mode=clientless&state=disconnected');
  await p.locator('.seg[aria-label="Operating mode"] button', { hasText: 'Client-assisted' }).click();
  check(await p.locator('.flash').innerText().then((t) => t.includes('End the run first')), 'mode switch is refused while a disconnected run is held');
  await p.keyboard.press('Meta+.');
  await p.locator('.seg[aria-label="Operating mode"] button', { hasText: 'Client-assisted' }).click();
  check(await p.locator('.sheet').isVisible(), 'after the held run ends, the mode switch asks with a sheet');
  await p.close();
}
{
  const p = await open('variant=A&mode=client&state=running');
  await p.locator('.stage').click();
  check(await p.locator('.kbd-badge.game').isVisible(), 'clicking the game gives it keyboard focus and the badge says so');
  check(await p.locator('.stage.kbd-game').isVisible(), 'the game focus ring is drawn around the game rectangle');
  await p.close();
}

{
  const p = await open('variant=A&mode=client&state=running&freeze=1&chrome=0');
  await p.locator('.lane .m.confirmed').last().hover();
  const r = await p.waitForSelector('.tip', { timeout: 2000 }).then(() => p.evaluate(() => {
    const t = document.querySelector('.tip').getBoundingClientRect(); const g = document.querySelector('.game').getBoundingClientRect();
    return { shown: true, overlap: t.left < g.right && t.right > g.left && t.top < g.bottom && t.bottom > g.top };
  }), () => ({ shown: false, overlap: false }));
  check(r.shown && !r.overlap, 'a lane mark tooltip under the game stays outside the game rectangle');
  await p.close();
}
for (const v of ['A', 'B', 'C']) {
  const p = await open(`variant=${v}&mode=clientless&state=connect&chrome=0`);
  await p.locator('.connect button', { hasText: 'Simulate a rejected password' }).click();
  await p.waitForSelector('.connect .banner', { timeout: 2000 }).catch(() => {});
  const r = await p.evaluate(() => [...document.querySelectorAll('.banner,.ticker[data-banner]')].filter((e) => e.textContent.includes('Sign-in failed'))
    .map((e) => ({ inForm: !!e.closest('.connect'), alert: e.getAttribute('role') === 'alert' })));
  check(r.length === 1 && r[0].inForm && r[0].alert, `${v}: a rejected password shows one alert, inside the sign-in form (found ${r.length})`);
  await p.close();
}
for (const [state, want] of [['error-rejected', 'Each mark is one action'], ['disconnected', 'Last known, no live game state']]) {
  const p = await open(`variant=A&mode=clientless&state=${state}&freeze=1&chrome=0`);
  const aside = await p.locator('.board .sec-title', { hasText: 'Last minute' }).innerText();
  check(aside.includes(want), `${state}: the lanes caption agrees with the freshness chip ("${want}")`);
  await p.close();
}
for (const q of ['variant=A', 'variant=B', 'variant=C&drawer=0']) {
  const p = await open(`${q}&mode=client&state=running&chrome=0`);
  await p.locator('.seg[aria-label="Operating mode"] button', { hasText: 'Clientless' }).click();
  await p.waitForSelector('.flash', { timeout: 2000 }).catch(() => {});
  const r = await p.evaluate(() => {
    const f = document.querySelector('.flash')?.getBoundingClientRect(); const g = document.querySelector('.game').getBoundingClientRect();
    return f ? { shown: true, overlap: f.left < g.right && f.right > g.left && f.top < g.bottom && f.bottom > g.top } : { shown: false, overlap: false };
  });
  check(r.shown && !r.overlap, `${q}: the refusal message shows outside the game rectangle`);
  await p.close();
}

{
  const p = await open('variant=A&mode=client&state=running&chrome=0');
  const view = () => p.evaluate(() => ({ rail: !!document.querySelector('.rail'), ticker: !!document.querySelector('.ticker'), minis: document.querySelectorAll('.toolbar .mini').length,
    controls: [...document.querySelectorAll('.toolbar .controls .btn')].map((b) => b.innerText.replace(/\s+/g, ' ').trim()), toggle: document.querySelector('[aria-controls="rail"]')?.innerText.replace(/\s+/g, ' ').trim(),
    game: document.querySelector('.game')?.getBoundingClientRect().width ?? 0, tab: document.querySelector('.tab[aria-selected="true"]')?.innerText ?? '' }));
  const full = await view();
  check(full.rail && !full.ticker && full.toggle === 'Hide panel ⌘\\', `A shows the side panel with a labeled Hide panel toggle (${full.toggle})`);
  await p.keyboard.press('Meta+Backslash');
  const hidden = await view();
  check(!hidden.rail && hidden.ticker && hidden.minis === 3 && hidden.toggle === 'Show panel ⌘\\', 'Cmd+\\ hides the panel and moves status to the toolbar and the ticker');
  check(hidden.controls.length === 2 && hidden.controls.every((t) => /⌘/.test(t) && /[A-Za-z]/.test(t)), `the hidden-panel toolbar keeps labeled run controls (${hidden.controls.join(', ')})`);
  check(hidden.game > full.game, `hiding the panel enlarges the game (${Math.round(full.game)} to ${Math.round(hidden.game)} px wide)`);
  await p.keyboard.press('Meta+2');
  const shown = await view();
  check(shown.rail && shown.tab.includes('Setup'), 'Cmd+2 shows the panel again on the Setup tab');
  await p.keyboard.press('Meta+Backslash');
  await p.locator('[aria-controls="rail"]').click({ timeout: 2000 }).catch(() => {});
  check(await p.locator('.rail').isVisible(), 'the Show panel button brings the panel back');
  await p.close();
}
{
  const p = await open('variant=A&mode=client&state=ready-blocked&rail=0&chrome=0');
  check(await p.locator('.toolbar .controls .btn.primary').isDisabled({ timeout: 2000 }).catch(() => false), 'Start is disabled while a preflight check blocks it');
  check(await p.locator('.ticker').innerText({ timeout: 2000 }).then((t) => t.includes('Start is unavailable. HP potions: None in the bag.'), () => false), 'with the panel hidden, the ticker says why Start is unavailable');
  await p.close();
}
for (const [q, want] of [['variant=A&rail=0', 0], ['variant=C&drawer=0', 1]]) {
  const p = await open(`${q}&mode=client&state=paused-potions&chrome=0`);
  const n = await p.locator('.ticker .btn', { hasText: /^Resume$/ }).count();
  check(n === want, `${q}: ${want ? 'the ticker carries a labeled Resume, because the toolbar controls are icon only' : 'the ticker leaves Resume to the labeled toolbar control'} (found ${n})`);
  await p.close();
}
{
  const p = await ctx.newPage();
  const theme = () => p.evaluate(() => document.documentElement.dataset.theme);
  const settles = (t) => p.waitForFunction((x) => document.documentElement.dataset.theme === x, t, { timeout: 2000 }).then(() => true, () => false);
  await p.emulateMedia({ colorScheme: 'light' });
  await p.goto(`${page0}?variant=A&mode=client&state=running&chrome=0`);
  await p.waitForSelector('.window');
  check(await theme() === 'light', 'with no theme chosen, the prototype opens in the light system appearance');
  await p.emulateMedia({ colorScheme: 'dark' });
  const toDark = await settles('dark');
  await p.emulateMedia({ colorScheme: 'light' });
  check(toDark && await settles('light'), 'a change of system appearance switches the theme live, both ways');
  await p.goto(`${page0}?variant=A&mode=client&state=running&chrome=0&theme=light`);
  await p.waitForSelector('.window');
  await p.emulateMedia({ colorScheme: 'dark' });
  await p.waitForTimeout(300);
  check(await theme() === 'light', 'a theme chosen in the prototype bar overrides the system appearance');
  await p.close();
}

check(errors.length === 0, `no page errors${errors.length ? ': ' + errors.join(' | ') : ''}`);
await browser.close();
const failed = results.filter((r) => !r.ok).length;
console.log(failed ? `${failed} check(s) failed` : `all ${results.length} checks pass, ${taken} screenshots in screenshots/`);
process.exit(failed ? 1 : 0);
