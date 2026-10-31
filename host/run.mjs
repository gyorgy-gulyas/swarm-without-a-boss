// Batch runner (Monte Carlo).
//
//   node host/run.mjs strategy.kind=0 env.failure_rate_per_h=0.01 --runs=100
//   node host/run.mjs strategy.kind=1 design.radio_range_m=100,200,300 --runs=50 --out=results/range.csv
//
// Every run uses a different seed (run.seed = 1..runs unless given). One
// parameter may be swept with a comma list. Prints a summary per value and,
// with --out, writes one CSV row per run.
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { loadEngine, parseArgs } from './node-engine.mjs';

const opts = parseArgs(process.argv.slice(2));
const eng = await loadEngine();
const values = opts.sweep ? opts.sweep.values : [null];
const rows = [];

const mean = (a) => a.reduce((s, x) => s + x, 0) / a.length;
const q = (a, f) => { const s = [...a].sort((x, y) => x - y); return s[Math.min(s.length - 1, Math.floor(f * s.length))]; };

for (const v of values) {
  const res = [];
  const t0 = performance.now();
  for (let k = 0; k < opts.runs; k++) {
    const set = { ...opts.set, 'run.seed': (opts.set['run.seed'] ?? 1) + k };
    if (opts.sweep) set[opts.sweep.name] = v;
    eng.init(set);
    const m = eng.runToEnd();
    res.push(m);
    rows.push({ ...set, ...m });
  }
  const ms = (performance.now() - t0) / opts.runs;
  const n = res[0].n_points;
  const label = opts.sweep ? `${opts.sweep.name}=${v}` : 'run';
  const pct = (x) => (100 * x / n).toFixed(2) + '%';
  console.log(
    `${label}  runs=${opts.runs}  ` +
    `miss=${pct(mean(res.map((r) => r.n_miss)))} (p90 ${pct(q(res.map((r) => r.n_miss), 0.9))})  ` +
    `dup=${pct(mean(res.map((r) => r.n_dup)))}  ` +
    `J=${mean(res.map((r) => r.J)).toFixed(0)}  ` +
    `t_end=${(mean(res.map((r) => r.t_last_measure_s)) / 3600).toFixed(1)} h  ` +
    `alive=${mean(res.map((r) => r.alive)).toFixed(1)}  ` +
    `msgs=${mean(res.map((r) => r.msgs)).toFixed(0)}  ` +
    `[${ms.toFixed(0)} ms/run]`,
  );
}

if (opts.out) {
  mkdirSync(dirname(opts.out), { recursive: true });
  const cols = Object.keys(rows[0]);
  writeFileSync(opts.out, [cols.join(','), ...rows.map((r) => cols.map((c) => r[c]).join(','))].join('\n') + '\n');
  console.log(`wrote ${opts.out} (${rows.length} rows)`);
}
