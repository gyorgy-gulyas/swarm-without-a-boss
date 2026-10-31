// Thin JavaScript wrapper around the WebAssembly engine.
// Works the same in Node.js and in the browser: the caller supplies the
// .wasm bytes, everything else is plain WebAssembly.
//
// One Engine = one WebAssembly instance = one simulation at a time.

export class Engine {
  static async load(wasmBytes) {
    const { instance } = await WebAssembly.instantiate(wasmBytes, {});
    return new Engine(instance.exports);
  }

  constructor(x) {
    this.x = x;
    this.paramNames = this.#str(x.param_names_ptr(), x.param_names_len()).split(',');
    this.metricNames = this.#str(x.metric_names_ptr(), x.metric_names_len()).split(',');
    x.params_reset();
    this.defaults = this.params();
  }

  #mem() { return this.x.memory.buffer; }

  #str(ptr, len) {
    return new TextDecoder().decode(new Uint8Array(this.#mem(), ptr, len));
  }

  /** Current parameter values as { 'env.area_m': 1000, ... }. */
  params() {
    const v = new Float64Array(this.#mem(), this.x.params_ptr(), this.paramNames.length);
    return Object.fromEntries(this.paramNames.map((n, i) => [n, v[i]]));
  }

  /** Start a new run: defaults, overridden by `overrides`. Unknown names throw. */
  init(overrides = {}) {
    this.x.params_reset();
    const v = new Float64Array(this.#mem(), this.x.params_ptr(), this.paramNames.length);
    for (const [k, val] of Object.entries(overrides)) {
      const i = this.paramNames.indexOf(k);
      if (i < 0) throw new Error(`unknown parameter: ${k}`);
      v[i] = Number(val);
    }
    this.x.init();
  }

  /** Advance up to n steps; true when the run is over. */
  step(n = 1) { return this.x.step(n) === 1; }

  runToEnd() { this.x.run_to_end(); return this.metrics(); }

  time() { return this.x.time(); }

  metrics() {
    const ptr = this.x.metrics_update();
    const v = new Float64Array(this.#mem(), ptr, this.metricNames.length);
    return Object.fromEntries(this.metricNames.map((n, i) => [n, v[i]]));
  }

  /** [{x, y, alive, measuring}] */
  rovers() {
    const ptr = this.x.rovers_update();
    const n = this.x.rover_count();
    const v = new Float32Array(this.#mem(), ptr, n * 4);
    const out = [];
    for (let i = 0; i < n; i++) {
      out.push({ x: v[i * 4], y: v[i * 4 + 1], alive: v[i * 4 + 2] > 0, measuring: v[i * 4 + 3] > 0 });
    }
    return out;
  }

  /** Measurement count per grid point (copy), plus the side length. */
  grid() {
    const side = this.x.grid_side();
    const counts = new Uint8Array(this.#mem(), this.x.grid_ptr(), side * side).slice();
    return { side, counts };
  }

  /** Radio links between live rovers: [[i, j], ...] */
  links() {
    const ptr = this.x.links_update();
    const n = this.x.links_len();
    const v = new Uint16Array(this.#mem(), ptr, n * 2);
    const out = [];
    for (let k = 0; k < n; k++) out.push([v[2 * k], v[2 * k + 1]]);
    return out;
  }
}
