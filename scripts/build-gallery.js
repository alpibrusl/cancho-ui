#!/usr/bin/env node
// Builds the landing-page gallery (docs/index.html, section #gallery) from docs/gallery.json:
//   - each "supported" tile's example is compiled with `cancho-ui build --page` (a page, not an installable app: docs/mobile-backend.md
//     section 5.2) into docs/demo/<id>/ (the ops-console tile also
//     gets a static sample of its service at docs/demo/ops-console/api/machines);
//   - the section's markup is written between the markers <!-- gallery:start --> and <!-- gallery:end -->.
// `node scripts/build-gallery.js` writes; `scripts/check-gallery.js` rebuilds into a temporary directory and fails on any drift.
'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');
const { machines } = require('./ops-console-server.js');

const ROOT = path.join(__dirname, '..');
const START = '<!-- gallery:start -->';
const END = '<!-- gallery:end -->';

function esc(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
// `code` spans in a manifest sentence become <code>.
function inline(s) {
  return esc(s).replace(/`([^`]+)`/g, '<code>$1</code>');
}

function loadManifest() {
  return JSON.parse(fs.readFileSync(path.join(ROOT, 'docs', 'gallery.json'), 'utf8'));
}

function renderSection(m) {
  const lines = [];
  lines.push(START);
  lines.push('<section class="alt" id="gallery"><div class="wrap">');
  lines.push('  <h2>Gallery</h2>');
  lines.push('  <p class="lead">Pages the compiler builds today, running here, and the kinds of page it cannot build yet.</p>');
  lines.push('  <h3 class="gh">Built with cancho-ui</h3>');
  lines.push('  <ul class="gallery">');
  for (const t of m.supported) {
    const src = `${m.repo}/blob/main/${t.source}`;
    lines.push(`    <li class="gtile${t.wide ? ' wide' : ''}"><h4>${esc(t.title)}</h4>`);
    lines.push(`      <p>${inline(t.says)}</p>`);
    lines.push(`      <iframe class="${t.tall ? 'tall' : ''}" title="${esc(t.title)}, built by cancho-ui" src="demo/${t.id}/" loading="lazy"></iframe>`);
    lines.push(`      <p class="glinks"><a href="demo/${t.id}/">Open on its own</a> <a href="${src}">Source: ${esc(path.basename(t.source))}</a></p></li>`);
  }
  lines.push('  </ul>');
  lines.push('  <h3 class="gh">Coming soon</h3>');
  lines.push('  <p class="lead">Not built, and not promised for any date. Each tile says what is missing and links the issue that tracks it.</p>');
  lines.push('  <ul class="gallery">');
  for (const t of m.soon) {
    lines.push(`    <li class="gtile soon"><h4>${esc(t.title)} <span class="badge">Coming soon</span></h4>`);
    lines.push(`      <p>Missing: ${inline(t.missing)}</p>`);
    lines.push(`      <p class="glinks"><a href="${m.repo}/issues/${t.issue}">Tracked in #${t.issue}</a></p></li>`);
  }
  lines.push('  </ul>');
  lines.push('</div></section>');
  lines.push(END);
  return lines.join('\n');
}

// Compile every supported example into `outRoot/<id>/`.
function buildDemos(m, cancho, outRoot) {
  for (const t of m.supported) {
    const dir = path.join(outRoot, t.id);
    fs.rmSync(dir, { recursive: true, force: true });
    // run from the repository root with the relative source path: the emitted code names its source in a comment
    const r = spawnSync(cancho, ['build', t.source, '--page', '-o', dir, '--name', t.title], { encoding: 'utf8', cwd: ROOT });
    if (r.status !== 0) { throw new Error(`building ${t.source}: ${r.stderr}${r.stdout}`); }
    if (t.id === 'ops-console') {
      fs.mkdirSync(path.join(dir, 'api'), { recursive: true });
      const text = machines().map((x) => JSON.stringify(x)).join('\n') + '\n';
      fs.writeFileSync(path.join(dir, 'api', 'machines'), text);
    }
  }
}

function splice(html, section) {
  const a = html.indexOf(START);
  const b = html.indexOf(END);
  if (a < 0 || b < 0 || b < a) { throw new Error('docs/index.html has no gallery markers'); }
  return html.slice(0, a) + section + html.slice(b + END.length);
}

module.exports = { loadManifest, renderSection, buildDemos, splice, START, END, ROOT };

if (require.main === module) {
  const cancho = process.argv[2] || path.join(ROOT, 'target', 'debug', 'cancho-ui');
  const m = loadManifest();
  buildDemos(m, cancho, path.join(ROOT, 'docs', 'demo'));
  const file = path.join(ROOT, 'docs', 'index.html');
  fs.writeFileSync(file, splice(fs.readFileSync(file, 'utf8'), renderSection(m)));
  console.log(`gallery: ${m.supported.length} demos built, ${m.soon.length} coming-soon tiles written`);
}
