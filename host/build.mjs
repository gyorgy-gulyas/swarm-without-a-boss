// Build the engine to WebAssembly and copy it to dist/engine.wasm,
// where both the Node.js runner and the browser viewer pick it up.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
execFileSync('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown'], {
  cwd: join(root, 'engine'),
  stdio: 'inherit',
});
mkdirSync(join(root, 'dist'), { recursive: true });
const src = join(root, 'engine', 'target', 'wasm32-unknown-unknown', 'release', 'swarm_engine.wasm');
const dst = join(root, 'dist', 'engine.wasm');
copyFileSync(src, dst);
console.log(`dist/engine.wasm  ${(statSync(dst).size / 1024).toFixed(0)} KB`);
