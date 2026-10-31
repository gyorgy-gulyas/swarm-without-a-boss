// Live viewer: the same WebAssembly engine as the Node.js runner, drawn on a canvas.
import { Engine } from '../host/engine.mjs';
import { PALETTE as P, css } from '../host/palette.mjs';

const canvas = document.getElementById('map');
const ctx = canvas.getContext('2d');
const form = document.getElementById('params');
const playBtn = document.getElementById('play');
const speed = document.getElementById('speed');
const stats = document.getElementById('stats');

const eng = await Engine.load(await (await fetch('../dist/engine.wasm')).arrayBuffer());
let playing = false;
let done = false;
let gridCanvas = null;

function readForm() {
  const set = {};
  for (const el of form.elements) {
    if (!el.name) continue;
    set[el.name] = Number(el.value);
    const out = el.parentElement.querySelector('output');
    if (out) out.textContent = el.value;
  }
  document.getElementById('speedOut').textContent = speed.value;
  return set;
}

function reset() {
  eng.init(readForm());
  done = false;
  draw();
}

function draw() {
  const { side, counts } = eng.grid();
  if (!gridCanvas || gridCanvas.width !== side) {
    gridCanvas = new OffscreenCanvas(side, side);
  }
  const g = gridCanvas.getContext('2d');
  const img = g.createImageData(side, side);
  for (let iy = 0; iy < side; iy++)
    for (let ix = 0; ix < side; ix++) {
      const c = counts[iy * side + ix];
      const col = c === 0 ? P.bg : c === 1 ? P.once : P.dup;
      const k = ((side - 1 - iy) * side + ix) * 4;
      img.data[k] = col[0]; img.data[k + 1] = col[1]; img.data[k + 2] = col[2]; img.data[k + 3] = 255;
    }
  g.putImageData(img, 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(gridCanvas, 0, 0, canvas.width, canvas.height);

  const area = eng.params()['env.area_m'];
  const cell = canvas.width / side;
  const s = (canvas.width - cell) / area;
  const X = (x) => (x + area / 2) * s + cell / 2;
  const Y = (y) => (area / 2 - y) * s + cell / 2;
  const rovers = eng.rovers();

  ctx.strokeStyle = css(P.link);
  ctx.globalAlpha = 0.55;
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (const [i, j] of eng.links()) {
    ctx.moveTo(X(rovers[i].x), Y(rovers[i].y));
    ctx.lineTo(X(rovers[j].x), Y(rovers[j].y));
  }
  ctx.stroke();
  ctx.globalAlpha = 1;

  ctx.strokeStyle = css(P.lander);
  ctx.lineWidth = 2;
  ctx.beginPath(); ctx.arc(X(0), Y(0), cell * 2.2, 0, Math.PI * 2); ctx.stroke();

  for (const r of rovers) {
    if (r.alive) {
      ctx.fillStyle = css(r.measuring ? P.measuring : P.rover);
      ctx.beginPath(); ctx.arc(X(r.x), Y(r.y), cell * 1.6, 0, Math.PI * 2); ctx.fill();
    } else {
      const d = cell * 1.4;
      ctx.strokeStyle = css(P.dead); ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(X(r.x) - d, Y(r.y) - d); ctx.lineTo(X(r.x) + d, Y(r.y) + d);
      ctx.moveTo(X(r.x) - d, Y(r.y) + d); ctx.lineTo(X(r.x) + d, Y(r.y) - d);
      ctx.stroke();
    }
  }

  const m = eng.metrics();
  const pct = (x) => (100 * x / m.n_points).toFixed(1) + ' %';
  const rows = [
    ['mission time', (m.t_s / 3600).toFixed(1) + ' h'],
    ['measured', pct(m.measured)],
    ['missed', done ? pct(m.n_miss) : '–'],
    ['duplicates', pct(m.n_dup)],
    ['J = c_miss·N_miss + c_dup·N_dup', done ? m.J.toFixed(0) : '–'],
    ['rovers alive', m.alive],
    ['radio messages', m.msgs.toLocaleString('en')],
    ['state', done ? 'finished' : playing ? 'running' : 'paused'],
  ];
  stats.innerHTML = rows.map(([k, v]) => `<dt>${k}</dt><dd>${v}</dd>`).join('');
}

function frame() {
  if (playing && !done) {
    const steps = Math.round(Number(speed.value) * 60 / eng.params()['run.dt_s']);
    done = eng.step(steps);
    if (done) { playing = false; playBtn.textContent = 'Play'; }
    draw();
  }
  requestAnimationFrame(frame);
}

playBtn.addEventListener('click', () => {
  if (done) reset();
  playing = !playing;
  playBtn.textContent = playing ? 'Pause' : 'Play';
  draw();
});
document.getElementById('reset').addEventListener('click', () => {
  playing = false; playBtn.textContent = 'Play'; reset();
});
form.addEventListener('input', () => { playing = false; playBtn.textContent = 'Play'; reset(); });
speed.addEventListener('input', readForm);

reset();
requestAnimationFrame(frame);
