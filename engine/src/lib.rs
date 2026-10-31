//! Simulation engine for a leaderless swarm of small survey rovers.
//!
//! The same code runs natively (`cargo test`) and as WebAssembly, both in
//! Node.js (batch runs) and in the browser (live viewer). The WebAssembly
//! interface is deliberately plain: no bindings generator, only numbers and
//! pointers into the module's memory, so any host can drive it.
//!
//! Host protocol:
//! 1. write parameter values into `params_ptr()` (order: `param_names`),
//!    or call `params_reset()` for the defaults;
//! 2. `init()`;
//! 3. `step(n)` until it returns 1, or `run_to_end()`;
//! 4. read `metrics_ptr()`, `rovers_ptr()`, `grid_ptr()`, `links_ptr()`
//!    after calling the matching `*_update()`.

pub mod brain;
pub mod params;
pub mod rng;
pub mod strategies;
pub mod world;

use params::{Params, DEFAULTS, MAX_PARAMS, NAMES};
use world::{World, METRIC_NAMES};

struct Host {
    params: [f64; MAX_PARAMS],
    world: Option<World>,
    metrics: [f64; 11],
    rovers: Vec<f32>,
    links: Vec<u16>,
    param_names: String,
    metric_names: String,
}

static mut HOST: Option<Host> = None;

#[allow(static_mut_refs)]
fn host() -> &'static mut Host {
    // Single-threaded by construction: one WebAssembly instance = one run.
    unsafe {
        HOST.get_or_insert_with(|| {
            let mut params = [0.0; MAX_PARAMS];
            params[..DEFAULTS.len()].copy_from_slice(DEFAULTS);
            Host {
                params,
                world: None,
                metrics: [0.0; 11],
                rovers: Vec::new(),
                links: Vec::new(),
                param_names: NAMES.join(","),
                metric_names: METRIC_NAMES.join(","),
            }
        })
    }
}

#[no_mangle]
pub extern "C" fn param_count() -> u32 {
    NAMES.len() as u32
}

#[no_mangle]
pub extern "C" fn param_names_ptr() -> *const u8 {
    host().param_names.as_ptr()
}

#[no_mangle]
pub extern "C" fn param_names_len() -> u32 {
    host().param_names.len() as u32
}

#[no_mangle]
pub extern "C" fn metric_names_ptr() -> *const u8 {
    host().metric_names.as_ptr()
}

#[no_mangle]
pub extern "C" fn metric_names_len() -> u32 {
    host().metric_names.len() as u32
}

#[no_mangle]
pub extern "C" fn params_ptr() -> *mut f64 {
    host().params.as_mut_ptr()
}

#[no_mangle]
pub extern "C" fn params_reset() {
    host().params[..DEFAULTS.len()].copy_from_slice(DEFAULTS);
}

#[no_mangle]
pub extern "C" fn init() {
    let h = host();
    h.world = Some(World::new(Params::from_slice(&h.params[..NAMES.len()])));
}

/// Advance at most `n` time steps. Returns 1 when the run is over.
#[no_mangle]
pub extern "C" fn step(n: u32) -> u32 {
    let Some(w) = host().world.as_mut() else { return 1 };
    for _ in 0..n {
        if w.step() {
            return 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn run_to_end() {
    if let Some(w) = host().world.as_mut() {
        w.run_to_end();
    }
}

#[no_mangle]
pub extern "C" fn time() -> f64 {
    host().world.as_ref().map_or(0.0, |w| w.t)
}

#[no_mangle]
pub extern "C" fn metrics_update() -> *const f64 {
    let h = host();
    if let Some(w) = h.world.as_ref() {
        h.metrics = w.metrics().to_array();
    }
    h.metrics.as_ptr()
}

/// 4 floats per rover: x, y, alive (0/1), measuring (0/1).
#[no_mangle]
pub extern "C" fn rovers_update() -> *const f32 {
    let h = host();
    h.rovers.clear();
    if let Some(w) = h.world.as_ref() {
        for r in &w.rovers {
            h.rovers.extend_from_slice(&[
                r.x as f32,
                r.y as f32,
                r.alive as u8 as f32,
                r.measuring.is_some() as u8 as f32,
            ]);
        }
    }
    h.rovers.as_ptr()
}

#[no_mangle]
pub extern "C" fn rover_count() -> u32 {
    host().world.as_ref().map_or(0, |w| w.rovers.len() as u32)
}

/// Measurement count per grid point, row by row (`grid_side()` × `grid_side()`).
#[no_mangle]
pub extern "C" fn grid_ptr() -> *const u8 {
    host().world.as_ref().map_or(std::ptr::null(), |w| w.counts.as_ptr())
}

#[no_mangle]
pub extern "C" fn grid_side() -> u32 {
    host().world.as_ref().map_or(0, |w| w.grid.side)
}

/// Radio links between live rovers, as pairs of rover indices.
#[no_mangle]
pub extern "C" fn links_update() -> *const u16 {
    let h = host();
    h.links.clear();
    if let Some(w) = h.world.as_ref() {
        for (a, b) in w.links() {
            h.links.push(a);
            h.links.push(b);
        }
    }
    h.links.as_ptr()
}

#[no_mangle]
pub extern "C" fn links_len() -> u32 {
    (host().links.len() / 2) as u32
}

#[cfg(test)]
mod tests {
    use super::params::{Params, StrategyKind};
    use super::world::World;

    fn small(strategy: StrategyKind) -> Params {
        let mut p = Params::defaults();
        p.area_m = 200.0;
        p.rovers = 5;
        p.strategy = strategy;
        p.deadline_h = 1000.0;
        p
    }

    #[test]
    fn stripes_without_failures_measure_everything_once() {
        let m = World::new(small(StrategyKind::Stripes)).run_to_end();
        assert_eq!(m.n_miss, 0);
        assert_eq!(m.n_dup, 0);
        assert_eq!(m.msgs, 0);
    }

    #[test]
    fn swarm_without_failures_misses_nothing() {
        let m = World::new(small(StrategyKind::Swarm)).run_to_end();
        assert_eq!(m.n_miss, 0);
    }

    #[test]
    fn swarm_covers_for_a_failed_rover() {
        let mut p = small(StrategyKind::Swarm);
        p.failure_rate_per_h = 0.5;
        p.seed = 7;
        let mut w = World::new(p);
        let m = w.run_to_end();
        if m.alive > 0 {
            assert_eq!(m.n_miss, 0, "{:?}", m);
        }
    }

    #[test]
    fn same_seed_same_result() {
        let mut p = small(StrategyKind::Swarm);
        p.failure_rate_per_h = 0.2;
        p.radio_range_m = 60.0;
        let a = World::new(p.clone()).run_to_end();
        let b = World::new(p).run_to_end();
        assert_eq!((a.n_miss, a.n_dup, a.t_s), (b.n_miss, b.n_dup, b.t_s));
    }
}
