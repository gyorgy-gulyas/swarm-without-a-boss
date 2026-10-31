# Swarm simulator

A simulator for a thought experiment: twenty-five small rovers survey a square
kilometre on the Moon, measuring something every five metres, with **no leader**.
Any rover may fail at any time, the radio does not reach everyone, and the
swarm has to cope on its own.

The simulator is being built step by step, together with an article series
that explains the reasoning behind every step:

- Hungarian: <https://ggyt.hu/fonok-nelkuli-raj.html>
- English: <https://ggyt.hu/en/swarm-without-a-boss.html>

This repository always holds the current state. Tags mark the state that
belongs to each part of the series (`part-02`, …).

## What is in this version

The world is deliberately simple: flat terrain, unlimited energy, every rover
knows its exact position. Only two things come from reality: **rovers fail**
(at a constant rate, without warning) and **the radio has a limited range**
(messages reach only rovers within range; anything further must be relayed).

Two strategies compete:

- **stripes** – every rover gets a fixed band of the grid before launch and
  walks it in a serpentine. No radio needed, no duplicates, but a failed
  rover's band stays unmeasured.
- **swarm** – the simplest leaderless rule: go to the nearest point that is
  neither measured nor claimed, claim it, tell the neighbours. Claims expire
  unless renewed, so a failed rover's work is picked up without anyone having
  to notice the failure. News spreads by flooding, and rovers occasionally
  exchange their whole map so that separated groups can catch up.

## Architecture

```
engine/   Rust: the world (truth) and the rover brains, compiled to WebAssembly
host/     Node.js: build, batch runs (Monte Carlo), PNG snapshots, static server
viewer/   browser: live view of the same WebAssembly engine
```

The **world** knows the truth: true positions, who has failed, which messages
arrive, how many times each grid point was really measured. A **brain** is
the software of one rover and sees only what a real rover could sense. The
contract between them (`engine/src/brain.rs`) is small and fixed:

- in: `Observation` – time, own position, busy flag, finished measurement, received messages;
- out: `Action` – drive / measure / idle, plus an optional radio message.

A brain is never told that another rover has failed. It can only notice that
it no longer hears from it.

The WebAssembly interface is plain numbers and pointers, no bindings
generator, so the same `engine.wasm` runs in Node.js and in the browser
without changes.

## Running it

Requirements: Rust (stable) with the `wasm32-unknown-unknown` target, Node.js 20+.

```sh
rustup target add wasm32-unknown-unknown
npm run build            # -> dist/engine.wasm
npm test                 # engine unit tests (native)

# batch runs: one parameter may be swept with a comma list
node host/run.mjs strategy.kind=0 env.failure_rate_per_h=0,0.005,0.01,0.02 --runs=100
node host/run.mjs strategy.kind=1 design.radio_range_m=30,60,100,200 --runs=50 --out=results/range.csv

# snapshots of one run
node host/snapshot.mjs strategy.kind=1 env.failure_rate_per_h=0.01 --at=3,6,15,end --out=out/swarm

# live viewer
npm run serve            # http://localhost:8080/viewer/
```

## Parameters

Names carry their group as a prefix:

| group | meaning | in this version |
|---|---|---|
| `env.` | given by the Moon, we cannot change it | `area_m`, `failure_rate_per_h`, `radio_loss` |
| `design.` | decided on Earth, fixed after landing | `rovers`, `speed_mps`, `measure_s`, `radio_range_m` |
| `goal.` | what we want and how we weigh errors | `spacing_m`, `deadline_h`, `c_miss`, `c_dup` |
| `strategy.` | how the rovers behave | `kind` (0 stripes, 1 swarm), `lease_s`, `gossip_period_s` |
| `run.` | the experiment itself | `seed`, `dt_s` |

The score of a run is `J = c_miss · N_miss + c_dup · N_dup`: missed points
are much worse than points measured twice. Every run is deterministic for a
given `run.seed`.

## License

MIT, see [LICENSE](LICENSE).

## Author

György Tamás Gulyás – <https://ggyt.hu>
