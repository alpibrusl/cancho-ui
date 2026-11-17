#!/usr/bin/env node
// Mutation test of the formats and the fit of `Photo` (docs/structure.md 12.0r): each mutant changes one thing in the source of the compiler or of the page's script, the compiler
// is built again, and the checks must fail (scripts/check-photo-formats.js, `cargo test` of the photo tests of cancho-ui and cancho-ui-ir). A mutant that no check kills is a hole
// in the checks, and is printed. The source is put back after every mutant, whatever happens.
//   node scripts/check-photo-formats-mutants.js
'use strict';
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');
const ROOT = path.join(__dirname, '..');
const JS = 'crates/cancho-ui-codegen-web/src/photo.rs';
const IR = 'crates/cancho-ui-ir/src/photo.rs';
const M = [
  ['the page does not know a PNG', JS, 'if (b.slice(0, 8) === "\\x89PNG\\r\\n\\x1a\\n") { return "image/png"; }', ''],
  ['the page does not know a WebP', JS, 'if (b.slice(0, 4) === "RIFF" && b.slice(8, 12) === "WEBP") { return "image/webp"; }', ''],
  ['the page takes any RIFF for a WebP', JS, 'b.slice(0, 4) === "RIFF" && b.slice(8, 12) === "WEBP"', 'b.slice(0, 4) === "RIFF"'],
  ['the page looks at seven bytes of a PNG signature', JS, 'b.slice(0, 8) === "\\x89PNG', 'b.slice(0, 7) === "\\x89PNG'],
  ['the page takes any text for a picture', JS, '&& __cui_photo_kind(t) !== ""; }', '; }'],
  ['the page lets the bounds go (no 1:2)', JS, 'w >= 2 * h ? [2, 1] : 2 * w <= h ? [1, 2] : [w, h]', 'w >= 2 * h ? [2, 1] : [w, h]'],
  ['the page lets the bounds go (no 2:1)', JS, 'w >= 2 * h ? [2, 1] : 2 * w <= h ? [1, 2] : [w, h]', '2 * w <= h ? [1, 2] : [w, h]'],
  ['the bound is 3:1, not 2:1', JS, 'w >= 2 * h ? [2, 1]', 'w >= 3 * h ? [2, 1]'],
  ['the box is not set from the header (it jumps when the picture is drawn)', JS, 'el.style.aspectRatio = d[0] + " / " + d[1]; }\n    p =', '}\n    p ='],
  ['the box is not set from the decoded picture', JS, 'if (fit) { el.style.aspectRatio = r[0] + " / " + r[1]; }', ''],
  ['a fitted picture is drawn on the stated ratio', JS, 'r = fit ? __cui_photo_ratio(bmp.width, bmp.height) : [w, h]', 'r = [w, h]'],
  ['a fitted picture is stretched, not scaled once', JS, 's = Math.max(cw / bmp.width, ch / bmp.height);\n        g.drawImage(bmp, (cw - bmp.width * s) / 2, (ch - bmp.height * s) / 2, bmp.width * s, bmp.height * s);', 'g.drawImage(bmp, 0, 0, cw, ch);'],
  ['a picture is fitted inside the canvas, not cut to cover it', JS, 's = Math.max(cw / bmp.width, ch / bmp.height);', 's = Math.min(cw / bmp.width, ch / bmp.height);'],
  ['the EXIF orientation is not applied to the header size', JS, 'return o >= 5 && o <= 8 ? [h, w] : [w, h];', 'return [w, h];'],
  ['only orientation 6 turns the picture', JS, 'o >= 5 && o <= 8', 'o === 6'],
  ['a thumbnail in the EXIF block is taken for the picture', JS, 'i += 2 + ((c(i + 2) << 8) | c(i + 3));', 'i += 2;'],
  ['the header of a lossless WebP is read short (13 bits of height)', JS, '(c(24) & 15) << 10', '(c(24) & 7) << 10'],
  ['the header of a lossy WebP is not masked to 14 bits', JS, 'w = (c(26) + (c(27) << 8)) & 16383;', 'w = (c(26) + (c(27) << 8));'],
  ['the width of a PNG is 16 bits', JS, 'w = c(16) * 16777216 + (c(17) << 16)', 'w = (c(17) << 16)'],
  ['fit is never written (the compiler drops it)', IR, 'find(bindings, "fit").and_then(bool_of) == Some(true)', 'false'],
  ['the compiler takes any text for a picture', IR, '        && picture_mime(text).is_some()\n}', '}'],
  ['the compiler does not know a WebP', IR, '} else if &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {', '} else if false {'],
  ['the compiler takes any RIFF for a WebP', IR, '&bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"', '&bytes[..4] == b"RIFF"'],
  ['the compiler looks at seven bytes of a PNG signature', IR, 'bytes[..8] == [0x89, b\'P\', b\'N\', b\'G\', 0x0D, 0x0A, 0x1A, 0x0A]', 'bytes[..7] == [0x89, b\'P\', b\'N\', b\'G\', 0x0D, 0x0A, 0x1A]'],
  ['replay takes a PNG from a field', IR, 'picture_ok(text) && picture_mime(text) == Some("image/jpeg")', 'picture_ok(text)'],
  ['fit: "yes" is accepted', IR, '"fit" => match bool_of(b) {\n                Some(_) => Ok(true),', '"fit" => match bool_of(b) {\n                Some(_) => Ok(true),\n                None if true => Ok(true),'],
];
// Measured equivalent: a mutant no real picture can tell from the original. The decoded picture sets the box again only for what the header reader cannot read or reads differently (a size
// the decoder corrects); no picture that Chromium decodes and the reader reads wrong was found, so there is no input that tells the two apart. It stays as a fallback.
const EQUIVALENT = new Set(['the box is not set from the decoded picture']);
const only = process.argv[2];
const run = (cmd, args) => spawnSync(cmd, args, { cwd: ROOT, encoding: 'utf8', env: process.env });
let survived = 0, n = 0;
for (const [name, file, from, to] of M) {
  if (only && !name.includes(only)) { continue; }
  n++;
  const p = path.join(ROOT, file), orig = fs.readFileSync(p, 'utf8');
  if (!orig.includes(from)) { console.log(`SKIP  ${name}: the text to change is not there`); survived++; continue; }
  try {
    fs.writeFileSync(p, orig.replace(from, to));
    const b = run('cargo', ['build', '--release', '-p', 'cancho-ui']);
    if (b.status !== 0) { console.log(`SKIP  ${name}: does not build`); survived++; continue; }
    const killers = [];
    if (run('node', ['scripts/check-photo-formats.js', 'target/release/cancho-ui']).status !== 0) { killers.push('formats'); }
    if (run('cargo', ['test', '--release', '-p', 'cancho-ui', '--test', 'photo']).status !== 0) { killers.push('cargo photo'); }
    if (!killers.length && run('cargo', ['test', '--release', '-p', 'cancho-ui', '--test', 'look', '--', 'photo', 'fit']).status !== 0) { killers.push('cargo look'); }
    if (!killers.length && run('cargo', ['test', '--release', '-p', 'cancho-ui-ir', 'photo']).status !== 0) { killers.push('ir'); }
    if (killers.length) { console.log(`killed ${name}  (${killers.join(', ')})`); } else if (EQUIVALENT.has(name)) { console.log(`equivalent ${name}`); } else { console.log(`SURVIVED ${name}`); survived++; }
  } finally { fs.writeFileSync(p, orig); }
}
run('cargo', ['build', '--release', '-p', 'cancho-ui']);
console.log(`\n${n} mutants, ${survived} not killed`);
process.exit(survived ? 1 : 0);
