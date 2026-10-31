// Run one simulation and save PNG snapshots at given mission times.
//
//   node host/snapshot.mjs strategy.kind=1 env.failure_rate_per_h=0.01 --at=2,10,20,end --out=out/swarm
//
// Writes out/swarm-2h.png, out/swarm-10h.png, ... (the map: dark = not yet
// measured, light = measured once, amber = measured twice or more; white dots
// are rovers, red crosses failed rovers, thin lines radio links).
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { loadEngine, parseArgs } from './node-engine.mjs';
import { Raster } from './png.mjs';
import { PALETTE as P } from './palette.mjs';

const opts = parseArgs(process.argv.slice(2));
const at = String(opts.at ?? 'end').split(',');
const out = opts.out ?? 'out/snapshot';
const cell = Number(opts.cell ?? 4);

const eng = await loadEngine();
eng.init(opts.set);
const area = eng.params()['env.area_m'];

function render(file) {
  const { side, counts } = eng.grid();
  const W = side * cell;
  const img = new Raster(W, W, P.bg);
  for (let iy = 0; iy < side; iy++)
    for (let ix = 0; ix < side; ix++) {
      const c = counts[iy * side + ix];
      if (c === 0) continue;
      // y grows upwards on the map
      img.fillRect(ix * cell, (side - 1 - iy) * cell, cell, cell, c === 1 ? P.once : P.dup);
    }
  const s = (W - cell) / area;
  const X = (x) => (x + area / 2) * s + cell / 2;
  const Y = (y) => (area / 2 - y) * s + cell / 2;
  const rovers = eng.rovers();
  for (const [i, j] of eng.links()) img.line(X(rovers[i].x), Y(rovers[i].y), X(rovers[j].x), Y(rovers[j].y), P.link, 0.55);
  img.ring(X(0), Y(0), cell * 2.2, P.lander, 2);
  for (const r of rovers) {
    if (r.alive) {
      img.disc(X(r.x), Y(r.y), cell * 1.6, r.measuring ? P.measuring : P.rover);
    } else {
      const d = cell * 1.4;
      for (let k = -1; k <= 1; k++) {
        img.line(X(r.x) - d, Y(r.y) - d + k, X(r.x) + d, Y(r.y) + d + k, P.dead);
        img.line(X(r.x) - d, Y(r.y) + d + k, X(r.x) + d, Y(r.y) - d + k, P.dead);
      }
    }
  }
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, img.toPNG());
  const m = eng.metrics();
  const pct = (x) => (100 * x / m.n_points).toFixed(1) + '%';
  console.log(`${file}  t=${(m.t_s / 3600).toFixed(1)} h  measured=${pct(m.measured)}  dup=${pct(m.n_dup)}  alive=${m.alive}`);
}

let done = false;
for (const a of at) {
  if (a === 'end') {
    if (!done) eng.runToEnd();
    render(`${out}-end.png`);
    continue;
  }
  const target = Number(a) * 3600;
  while (!done && eng.time() < target) done = eng.step(60);
  render(`${out}-${a}h.png`);
}
