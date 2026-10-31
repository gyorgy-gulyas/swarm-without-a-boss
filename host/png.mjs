// Minimal PNG writer (RGB, 8 bit) using only Node's built-in zlib.
import { deflateSync } from 'node:zlib';

const CRC = new Int32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c;
});
function crc32(buf) {
  let c = -1;
  for (const b of buf) c = CRC[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}

/** Canvas-like RGB raster with a few drawing helpers. */
export class Raster {
  constructor(w, h, bg = [0, 0, 0]) {
    this.w = w; this.h = h;
    this.px = new Uint8Array(w * h * 3);
    this.fillRect(0, 0, w, h, bg);
  }
  set(x, y, c, a = 1) {
    x |= 0; y |= 0;
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    const i = (y * this.w + x) * 3;
    for (let k = 0; k < 3; k++) this.px[i + k] = Math.round(this.px[i + k] * (1 - a) + c[k] * a);
  }
  fillRect(x, y, w, h, c) {
    for (let j = Math.max(0, y | 0); j < Math.min(this.h, (y + h) | 0); j++)
      for (let i = Math.max(0, x | 0); i < Math.min(this.w, (x + w) | 0); i++) this.set(i, j, c);
  }
  disc(cx, cy, r, c, a = 1) {
    for (let y = Math.floor(cy - r - 1); y <= cy + r + 1; y++)
      for (let x = Math.floor(cx - r - 1); x <= cx + r + 1; x++) {
        const d = Math.hypot(x + 0.5 - cx, y + 0.5 - cy);
        const cov = Math.max(0, Math.min(1, r + 0.5 - d));
        if (cov > 0) this.set(x, y, c, a * cov);
      }
  }
  ring(cx, cy, r, c, width = 1, a = 1) {
    for (let y = Math.floor(cy - r - width); y <= cy + r + width; y++)
      for (let x = Math.floor(cx - r - width); x <= cx + r + width; x++) {
        const d = Math.abs(Math.hypot(x + 0.5 - cx, y + 0.5 - cy) - r);
        const cov = Math.max(0, Math.min(1, width / 2 + 0.5 - d));
        if (cov > 0) this.set(x, y, c, a * cov);
      }
  }
  line(x0, y0, x1, y1, c, a = 1) {
    const n = Math.ceil(Math.hypot(x1 - x0, y1 - y0) * 2) + 1;
    for (let k = 0; k <= n; k++) {
      const t = k / n;
      this.set(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, c, a);
    }
  }
  toPNG() {
    const raw = Buffer.alloc((this.w * 3 + 1) * this.h);
    for (let y = 0; y < this.h; y++) {
      raw[y * (this.w * 3 + 1)] = 0;
      Buffer.from(this.px.buffer, y * this.w * 3, this.w * 3).copy(raw, y * (this.w * 3 + 1) + 1);
    }
    const ihdr = Buffer.alloc(13);
    ihdr.writeUInt32BE(this.w, 0); ihdr.writeUInt32BE(this.h, 4);
    ihdr[8] = 8; ihdr[9] = 2; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
    return Buffer.concat([
      Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
      chunk('IHDR', ihdr),
      chunk('IDAT', deflateSync(raw, { level: 9 })),
      chunk('IEND', Buffer.alloc(0)),
    ]);
  }
}
