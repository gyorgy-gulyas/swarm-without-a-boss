// Live viewer: the same WebAssembly engine as the Node.js runner, drawn on a canvas.
//
// Layers, bottom to top: lunar terrain (cosmetic only: the simulated terrain is
// flat), the measurement grid (one dot per grid point), radio links, rover
// trails, the lander and the rovers. Rovers and links are drawn at a fixed
// screen size, so they stay visible at every zoom level.
import { Engine } from '../host/engine.mjs';

const COL = {
  todo: 'rgba(18, 20, 26, 0.42)',
  measured: 'rgba(95, 212, 232, 0.9)',
  dup: '#ff9f43',
  rover: '#f7f3ea',
  outline: '#12151c',
  accent: '#ffd166',
  dead: '#ff5d5d',
  link: 'rgba(150, 200, 255, 0.55)',
  trail: [247, 243, 234],
};
const LAYER_K = 8;          // grid layer: pixels per grid cell
const TRAIL = 14;           // trail length, frames

const canvas = document.getElementById('map');
const ctx = canvas.getContext('2d');
const form = document.getElementById('params');
const playBtn = document.getElementById('play');
const speed = document.getElementById('speed');
const stats = document.getElementById('stats');

const eng = await Engine.load(await (await fetch('../dist/engine.wasm')).arrayBuffer());

let playing = false, done = false;
let grid = null;            // { side, spacing, half, counts, layer, prev }
let trails = [];
const view = { z: 1, vx: 0, vy: 0 };
let terrain = null;

// ---------- terrain (cosmetic) ----------

function makeTerrain(size = 1024) {
  const c = new OffscreenCanvas(size, size);
  const g = c.getContext('2d');
  const img = g.createImageData(size, size);
  const hash = (i, j, o) => {
    let h = (i * 374761393 + j * 668265263 + o * 2147483647) | 0;
    h = Math.imul(h ^ (h >>> 13), 1274126177);
    return ((h ^ (h >>> 16)) >>> 0) / 4294967295;
  };
  const smooth = (t) => t * t * (3 - 2 * t);
  const noise = (x, y, o) => {
    const i = Math.floor(x), j = Math.floor(y), fx = smooth(x - i), fy = smooth(y - j);
    const a = hash(i, j, o), b = hash(i + 1, j, o), c2 = hash(i, j + 1, o), d = hash(i + 1, j + 1, o);
    return a + (b - a) * fx + (c2 - a) * fy + (a - b - c2 + d) * fx * fy;
  };
  const lo = [104, 102, 98], hi = [158, 155, 148];
  for (let y = 0; y < size; y++)
    for (let x = 0; x < size; x++) {
      let n = 0, amp = 0.5, f = 3 / size, tot = 0;
      for (let o = 0; o < 6; o++) { n += amp * noise(x * f, y * f, o); tot += amp; amp *= 0.55; f *= 2.1; }
      n /= tot;
      const grain = (hash(x, y, 99) - 0.5) * 10;
      const k = (y * size + x) * 4;
      for (let ch = 0; ch < 3; ch++) img.data[k + ch] = lo[ch] + (hi[ch] - lo[ch]) * n + grain;
      img.data[k + 3] = 255;
    }
  g.putImageData(img, 0, 0);
  return c;
}

// ---------- grid layer ----------

function dot(g, ix, iy, side, c) {
  const x = ix * LAYER_K + LAYER_K / 2, y = (side - 1 - iy) * LAYER_K + LAYER_K / 2;
  g.clearRect(ix * LAYER_K, (side - 1 - iy) * LAYER_K, LAYER_K, LAYER_K);
  g.beginPath();
  if (c === 0) { g.fillStyle = COL.todo; g.arc(x, y, 1.3, 0, Math.PI * 2); }
  else if (c === 1) { g.fillStyle = COL.measured; g.arc(x, y, 1.7, 0, Math.PI * 2); }
  else { g.fillStyle = COL.dup; g.arc(x, y, 2.4, 0, Math.PI * 2); }
  g.fill();
}

function initGrid() {
  const p = eng.params();
  const { side, counts } = eng.grid();
  const layer = new OffscreenCanvas(side * LAYER_K, side * LAYER_K);
  const g = layer.getContext('2d');
  for (let iy = 0; iy < side; iy++) for (let ix = 0; ix < side; ix++) dot(g, ix, iy, side, 0);
  grid = { side, spacing: p['goal.spacing_m'], half: (side - 1) * p['goal.spacing_m'] / 2, counts, layer, prev: new Uint8Array(side * side) };
}

function updateGrid() {
  const { side, counts } = eng.grid();
  grid.counts = counts;
  const g = grid.layer.getContext('2d');
  for (let i = 0; i < counts.length; i++) {
    const c = Math.min(counts[i], 2);
    if (c !== grid.prev[i]) { dot(g, i % side, Math.floor(i / side), side, c); grid.prev[i] = c; }
  }
}

// ---------- view transform ----------

function cssSize() { return canvas.clientWidth; }
function scale() { return (cssSize() / (grid.side * grid.spacing)) * view.z; }
function X(x) { return cssSize() / 2 + (x - view.vx) * scale(); }
function Y(y) { return cssSize() / 2 - (y - view.vy) * scale(); }

function clampView() {
  view.z = Math.min(40, Math.max(1, view.z));
  const lim = grid.half * (1 - 1 / view.z) + grid.spacing / 2 * (1 - 1 / view.z);
  view.vx = Math.min(lim, Math.max(-lim, view.vx));
  view.vy = Math.min(lim, Math.max(-lim, view.vy));
}

function resize() {
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(canvas.clientWidth * dpr);
  canvas.height = Math.round(canvas.clientHeight * dpr);
  draw();
}

// ---------- drawing ----------

function draw() {
  if (!grid) return;
  const dpr = window.devicePixelRatio || 1;
  const S = cssSize(), s = scale();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = '#0d1016';
  ctx.fillRect(0, 0, S, S);

  // terrain and grid share the rectangle of the grid cells
  const ext = grid.half + grid.spacing / 2;
  const left = X(-ext), top = Y(ext), size = 2 * ext * s;
  ctx.imageSmoothingEnabled = true;
  ctx.drawImage(terrain, left, top, size, size);

  if (s * grid.spacing < LAYER_K * 1.2) {
    ctx.drawImage(grid.layer, left, top, size, size);
  } else {
    // zoomed in: draw the visible dots directly, crisp at any scale
    const sp = grid.spacing, side = grid.side;
    const toI = (v) => Math.round((v + grid.half) / sp);
    const ix0 = Math.max(0, toI(view.vx - S / 2 / s) - 1), ix1 = Math.min(side - 1, toI(view.vx + S / 2 / s) + 1);
    const iy0 = Math.max(0, toI(view.vy - S / 2 / s) - 1), iy1 = Math.min(side - 1, toI(view.vy + S / 2 / s) + 1);
    const r = Math.min(sp * s * 0.13, 6);
    for (const [c, col, rr] of [[0, COL.todo, r * 0.6], [1, COL.measured, r], [2, COL.dup, r * 1.2]]) {
      ctx.fillStyle = col;
      ctx.beginPath();
      for (let iy = iy0; iy <= iy1; iy++)
        for (let ix = ix0; ix <= ix1; ix++) {
          if (Math.min(grid.counts[iy * side + ix], 2) !== c) continue;
          const x = X(ix * sp - grid.half), y = Y(iy * sp - grid.half);
          ctx.moveTo(x + rr, y);
          ctx.arc(x, y, rr, 0, Math.PI * 2);
        }
      ctx.fill();
    }
  }

  const rovers = eng.rovers();

  // radio links
  ctx.strokeStyle = COL.link;
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (const [i, j] of eng.links()) {
    ctx.moveTo(X(rovers[i].x), Y(rovers[i].y));
    ctx.lineTo(X(rovers[j].x), Y(rovers[j].y));
  }
  ctx.stroke();

  // trails
  ctx.lineWidth = 1.5;
  ctx.lineCap = 'round';
  trails.forEach((tr, k) => {
    if (!rovers[k]?.alive) return;
    for (let i = 1; i < tr.length; i++) {
      ctx.strokeStyle = `rgba(${COL.trail.join(',')},${(0.35 * i) / tr.length})`;
      ctx.beginPath();
      ctx.moveTo(X(tr[i - 1][0]), Y(tr[i - 1][1]));
      ctx.lineTo(X(tr[i][0]), Y(tr[i][1]));
      ctx.stroke();
    }
  });

  // lander
  const lx = X(0), ly = Y(0);
  ctx.save();
  ctx.translate(lx, ly); ctx.rotate(Math.PI / 4);
  ctx.fillStyle = COL.accent; ctx.strokeStyle = COL.outline; ctx.lineWidth = 2;
  ctx.fillRect(-6, -6, 12, 12); ctx.strokeRect(-6, -6, 12, 12);
  ctx.restore();

  // rovers
  for (const r of rovers) {
    const x = X(r.x), y = Y(r.y);
    if (!r.alive) {
      for (const [col, w] of [[COL.outline, 5], [COL.dead, 2.5]]) {
        ctx.strokeStyle = col; ctx.lineWidth = w;
        ctx.beginPath();
        ctx.moveTo(x - 5, y - 5); ctx.lineTo(x + 5, y + 5);
        ctx.moveTo(x - 5, y + 5); ctx.lineTo(x + 5, y - 5);
        ctx.stroke();
      }
      continue;
    }
    ctx.shadowColor = 'rgba(0,0,0,0.6)'; ctx.shadowBlur = 6;
    ctx.fillStyle = COL.rover; ctx.strokeStyle = COL.outline; ctx.lineWidth = 1.6;
    ctx.beginPath(); ctx.arc(x, y, 5.2, 0, Math.PI * 2); ctx.fill();
    ctx.shadowBlur = 0;
    ctx.stroke();
    if (r.measuring) {
      ctx.fillStyle = COL.accent;
      ctx.beginPath(); ctx.arc(x, y, 2.4, 0, Math.PI * 2); ctx.fill();
    }
  }

  hud();
}

function hud() {
  const m = eng.metrics();
  const frac = m.measured / m.n_points;
  document.getElementById('clock').textContent = `T + ${(m.t_s / 3600).toFixed(1)} h`;
  document.getElementById('progress').style.width = `${(100 * frac).toFixed(1)}%`;
  document.getElementById('progressText').textContent =
    `${(100 * frac).toFixed(1)} % measured · ${m.alive} of ${eng.rovers().length} rovers alive`;
  const pct = (x) => (100 * x / m.n_points).toFixed(2) + ' %';
  const rows = [
    ['Mission time', (m.t_s / 3600).toFixed(1) + ' h'],
    ['Measured', pct(m.measured)],
    ['Missed', done ? pct(m.n_miss) : '—'],
    ['Measured twice', pct(m.n_dup)],
    ['Score J (lower is better)', done ? Math.round(m.J).toLocaleString('en') : '—'],
    ['Radio messages', m.msgs.toLocaleString('en')],
    ['State', done ? 'finished' : playing ? 'running' : 'paused'],
  ];
  stats.innerHTML = rows.map(([k, v]) => `<dt>${k}</dt><dd>${v}</dd>`).join('');
}

// ---------- control ----------

function readForm() {
  const set = {};
  for (const el of form.elements) {
    if (!el.name || (el.type === 'radio' && !el.checked)) continue;
    set[el.name] = Number(el.value);
    const out = el.closest('label')?.querySelector('output');
    if (out) out.textContent = el.value;
  }
  document.getElementById('speedOut').textContent = `${speed.value} min / frame`;
  return set;
}

function reset() {
  eng.init(readForm());
  done = false;
  initGrid();
  trails = eng.rovers().map((r) => [[r.x, r.y]]);
  clampView();
  draw();
}

function setPlaying(p) {
  playing = p;
  playBtn.textContent = playing ? 'Pause' : done ? 'Replay' : 'Play';
}

function frame() {
  if (playing && !done) {
    const steps = Math.round((Number(speed.value) * 60) / eng.params()['run.dt_s']);
    done = eng.step(steps);
    updateGrid();
    eng.rovers().forEach((r, k) => {
      trails[k].push([r.x, r.y]);
      if (trails[k].length > TRAIL) trails[k].shift();
    });
    if (done) setPlaying(false);
    draw();
  }
  requestAnimationFrame(frame);
}

playBtn.addEventListener('click', () => {
  if (done) reset();
  setPlaying(!playing);
  draw();
});
document.getElementById('reset').addEventListener('click', () => { setPlaying(false); reset(); });
form.addEventListener('input', () => { setPlaying(false); reset(); });
speed.addEventListener('input', readForm);

// zoom and pan
canvas.addEventListener('wheel', (e) => {
  e.preventDefault();
  const rect = canvas.getBoundingClientRect();
  const mx = e.clientX - rect.left, my = e.clientY - rect.top;
  const s0 = scale();
  const wx = view.vx + (mx - cssSize() / 2) / s0, wy = view.vy - (my - cssSize() / 2) / s0;
  view.z *= Math.exp(-e.deltaY * 0.0015);
  clampView();
  const s1 = scale();
  view.vx = wx - (mx - cssSize() / 2) / s1;
  view.vy = wy + (my - cssSize() / 2) / s1;
  clampView();
  draw();
}, { passive: false });
let drag = null;
canvas.addEventListener('pointerdown', (e) => { drag = { x: e.clientX, y: e.clientY }; canvas.classList.add('dragging'); canvas.setPointerCapture(e.pointerId); });
canvas.addEventListener('pointermove', (e) => {
  if (!drag) return;
  const s = scale();
  view.vx -= (e.clientX - drag.x) / s;
  view.vy += (e.clientY - drag.y) / s;
  drag = { x: e.clientX, y: e.clientY };
  clampView();
  draw();
});
canvas.addEventListener('pointerup', () => { drag = null; canvas.classList.remove('dragging'); });
canvas.addEventListener('dblclick', () => { view.z = 1; view.vx = 0; view.vy = 0; draw(); });
new ResizeObserver(resize).observe(canvas);

terrain = makeTerrain();
reset();
resize();
requestAnimationFrame(frame);
