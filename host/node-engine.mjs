// Load the engine from dist/engine.wasm in Node.js.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Engine } from './engine.mjs';

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');

let bytes;
export async function loadEngine() {
  bytes ??= readFileSync(join(ROOT, 'dist', 'engine.wasm'));
  return Engine.load(bytes);
}

/** Parse `name=value` arguments; a value may be a comma list (a sweep). */
export function parseArgs(argv) {
  const opts = { set: {}, sweep: null, runs: 1, out: null };
  for (const a of argv) {
    const m = a.match(/^--(\w+)(?:=(.*))?$/);
    if (m) { opts[m[1]] = m[2] ?? true; continue; }
    const [k, v] = a.split('=');
    if (v === undefined) throw new Error(`expected name=value, got: ${a}`);
    if (v.includes(',')) {
      if (opts.sweep) throw new Error('only one swept parameter at a time');
      opts.sweep = { name: k, values: v.split(',').map(Number) };
    } else {
      opts.set[k] = Number(v);
    }
  }
  opts.runs = Number(opts.runs);
  return opts;
}
