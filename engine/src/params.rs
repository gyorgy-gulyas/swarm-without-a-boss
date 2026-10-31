//! Simulation parameters.
//!
//! The prefix of every name tells its group:
//! - `env.`      what the Moon gives us and we cannot change (area, failures, radio loss)
//! - `design.`   decided on Earth, fixed after landing (rovers, speed, radio range)
//! - `goal.`     what we want and how we weigh errors (grid, deadline, costs)
//! - `strategy.` how the rovers behave: the software we are looking for
//! - `run.`      the experiment itself, not the Moon (seed, time step)
//!
//! The host (Node.js or the browser) passes the values as a flat f64 array in
//! the order of `NAMES`. The names are exported too, so the host never has to
//! hard-code the order.

pub const NAMES: &[&str] = &[
    "env.area_m",
    "env.failure_rate_per_h",
    "env.radio_loss",
    "design.rovers",
    "design.speed_mps",
    "design.measure_s",
    "design.radio_range_m",
    "goal.spacing_m",
    "goal.deadline_h",
    "goal.c_miss",
    "goal.c_dup",
    "strategy.kind",
    "strategy.lease_s",
    "strategy.gossip_period_s",
    "run.seed",
    "run.dt_s",
];

pub const DEFAULTS: &[f64] = &[
    1000.0,      // env.area_m: side of the square area
    0.0,         // env.failure_rate_per_h: per rover, constant rate
    0.0,         // env.radio_loss: probability that one delivery is lost
    25.0,        // design.rovers
    5.0 / 3.6,   // design.speed_mps: 5 km/h
    60.0,        // design.measure_s
    300.0,       // design.radio_range_m
    5.0,         // goal.spacing_m
    48.0,        // goal.deadline_h
    10.0,        // goal.c_miss
    1.0,         // goal.c_dup
    1.0,         // strategy.kind: 0 = stripes, 1 = swarm (greedy + lease)
    600.0,       // strategy.lease_s
    30.0,        // strategy.gossip_period_s
    1.0,         // run.seed
    1.0,         // run.dt_s
];

pub const MAX_PARAMS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StrategyKind {
    Stripes,
    Swarm,
}

#[derive(Clone, Debug)]
pub struct Params {
    pub area_m: f64,
    pub failure_rate_per_h: f64,
    pub radio_loss: f64,
    pub rovers: usize,
    pub speed_mps: f64,
    pub measure_s: f64,
    pub radio_range_m: f64,
    pub spacing_m: f64,
    pub deadline_h: f64,
    pub c_miss: f64,
    pub c_dup: f64,
    pub strategy: StrategyKind,
    pub lease_s: f64,
    pub gossip_period_s: f64,
    pub seed: u64,
    pub dt_s: f64,
}

impl Params {
    pub fn from_slice(v: &[f64]) -> Params {
        let g = |i: usize| v.get(i).copied().unwrap_or(DEFAULTS[i]);
        Params {
            area_m: g(0),
            failure_rate_per_h: g(1),
            radio_loss: g(2),
            rovers: g(3).max(1.0) as usize,
            speed_mps: g(4),
            measure_s: g(5),
            radio_range_m: g(6),
            spacing_m: g(7),
            deadline_h: g(8),
            c_miss: g(9),
            c_dup: g(10),
            strategy: if g(11) < 0.5 { StrategyKind::Stripes } else { StrategyKind::Swarm },
            lease_s: g(12),
            gossip_period_s: g(13),
            seed: g(14) as u64,
            dt_s: g(15),
        }
    }

    pub fn defaults() -> Params {
        Params::from_slice(DEFAULTS)
    }
}
