'use strict';
// A stand-in map tile: a 256 x 256 PNG that says which tile it is by its colour (the parity of x + y), with a border and a dot in the middle.
// It is what the examples and the checks serve instead of a tile provider's pictures: nothing in this repository asks a public tile server
// for anything (docs/maps.md).
const zlib = require('zlib');

const CRC = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) { c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; } t[n] = c >>> 0; }
  return t;
})();
function crc32(buf) { let c = 0xffffffff; for (const b of buf) { c = CRC[(c ^ b) & 0xff] ^ (c >>> 8); } return (c ^ 0xffffffff) >>> 0; }
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}
function tilePng(x, y, z) {
  const size = 256;
  const base = (x + y) % 2 === 0 ? [0xe6, 0xed, 0xf3] : [0xd3, 0xdc, 0xe5];
  const rows = [];
  for (let r = 0; r < size; r++) {
    const row = Buffer.alloc(1 + size * 3);
    for (let c = 0; c < size; c++) {
      const edge = r < 2 || c < 2 || r >= size - 2 || c >= size - 2;
      const dot = Math.abs(r - 128) < 4 && Math.abs(c - 128) < 4;
      // a faint grid of 64 px, so that a picture made of tiles shows its seams
      const grid = r % 64 === 0 || c % 64 === 0;
      const rgb = edge ? [0x8c, 0x9a, 0xa8] : dot ? [0x57, 0x60, 0x6a] : grid ? base.map((v) => v - 10) : base;
      row[1 + c * 3] = rgb[0]; row[2 + c * 3] = rgb[1]; row[3 + c * 3] = rgb[2];
    }
    rows.push(row);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0); ihdr.writeUInt32BE(size, 4); ihdr[8] = 8; ihdr[9] = 2;
  void z;
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(Buffer.concat(rows), { level: 9 })), chunk('IEND', Buffer.alloc(0))]);
}
module.exports = { tilePng };
