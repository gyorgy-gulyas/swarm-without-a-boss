//! The world: the test harness that knows the truth.
//!
//! It moves the rovers, runs their measurements, decides who fails and when,
//! delivers radio messages within range, and counts how many times every grid
//! point was really measured. The brains never see any of this directly.

use std::rc::Rc;

use crate::brain::{Brain, Command, Grid, Message, Mission, Observation};
use crate::params::{Params, StrategyKind};
use crate::rng::Rng;
use crate::strategies::{stripes::Stripes, swarm::Swarm};

pub struct Rover {
    pub x: f64,
    pub y: f64,
    pub alive: bool,
    pub fail_at: f64,
    pub measuring: Option<u32>,
    pub busy_until: f64,
    finished: Option<u32>,
    inbox: Vec<Rc<Message>>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Metrics {
    pub t_s: f64,
    pub n_points: u32,
    pub measured: u32,
    pub n_miss: u32,
    pub n_dup: u32,
    pub j: f64,
    pub t_last_measure_s: f64,
    pub alive: u32,
    pub msgs: u64,
    pub bytes: u64,
    pub finished: bool,
}

pub const METRIC_NAMES: &[&str] = &[
    "t_s", "n_points", "measured", "n_miss", "n_dup", "J", "t_last_measure_s", "alive", "msgs",
    "bytes", "finished",
];

impl Metrics {
    pub fn to_array(&self) -> [f64; 11] {
        [
            self.t_s,
            self.n_points as f64,
            self.measured as f64,
            self.n_miss as f64,
            self.n_dup as f64,
            self.j,
            self.t_last_measure_s,
            self.alive as f64,
            self.msgs as f64,
            self.bytes as f64,
            if self.finished { 1.0 } else { 0.0 },
        ]
    }
}

pub struct World {
    pub p: Params,
    pub grid: Grid,
    pub t: f64,
    pub rovers: Vec<Rover>,
    brains: Vec<Box<dyn Brain>>,
    /// How many times each grid point was really measured (saturates at 255).
    pub counts: Vec<u8>,
    rng: Rng,
    measured: u32,
    dups: u32,
    t_last_measure: f64,
    resting_since: Option<f64>,
    msgs: u64,
    bytes: u64,
    pub finished: bool,
}

impl World {
    pub fn new(p: Params) -> World {
        let grid = Grid::new(p.area_m, p.spacing_m);
        let mut rng = Rng::new(p.seed);
        let n = p.rovers;
        let mut rovers = Vec::with_capacity(n);
        let mut brains: Vec<Box<dyn Brain>> = Vec::with_capacity(n);
        for i in 0..n {
            // The rovers roll off the lander in different directions.
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            let fail_at = rng.exp(p.failure_rate_per_h) * 3600.0;
            rovers.push(Rover {
                x: 3.0 * a.cos(),
                y: 3.0 * a.sin(),
                alive: true,
                fail_at,
                measuring: None,
                busy_until: 0.0,
                finished: None,
                inbox: Vec::new(),
            });
            let m = Mission {
                id: i as u16,
                rovers: n,
                area_m: p.area_m,
                spacing_m: p.spacing_m,
                speed_mps: p.speed_mps,
                measure_s: p.measure_s,
                lease_s: p.lease_s,
                gossip_period_s: p.gossip_period_s,
            };
            brains.push(match p.strategy {
                StrategyKind::Stripes => Box::new(Stripes::new(&m)),
                StrategyKind::Swarm => Box::new(Swarm::new(&m)),
            });
        }
        let counts = vec![0u8; grid.len()];
        World {
            p,
            grid,
            t: 0.0,
            rovers,
            brains,
            counts,
            rng,
            measured: 0,
            dups: 0,
            t_last_measure: 0.0,
            resting_since: None,
            msgs: 0,
            bytes: 0,
            finished: false,
        }
    }

    /// One time step. Returns true when the run is over.
    pub fn step(&mut self) -> bool {
        if self.finished {
            return true;
        }
        let t = self.t;
        let dt = self.p.dt_s;

        // Failures and finished measurements.
        for r in self.rovers.iter_mut() {
            if !r.alive {
                continue;
            }
            if t >= r.fail_at {
                // A measurement in progress is lost with the rover.
                r.alive = false;
                r.measuring = None;
                r.inbox.clear();
                continue;
            }
            if let Some(p) = r.measuring {
                if t >= r.busy_until {
                    let c = &mut self.counts[p as usize];
                    if *c == 0 {
                        self.measured += 1;
                    } else {
                        self.dups += 1;
                    }
                    *c = c.saturating_add(1);
                    r.measuring = None;
                    r.finished = Some(p);
                    self.t_last_measure = t;
                }
            }
        }

        // Every live brain senses and acts.
        let mut outgoing: Vec<(usize, Rc<Message>)> = Vec::new();
        let step_len = self.p.speed_mps * dt;
        for i in 0..self.rovers.len() {
            if !self.rovers[i].alive {
                continue;
            }
            let inbox = std::mem::take(&mut self.rovers[i].inbox);
            let r = &mut self.rovers[i];
            let obs = Observation {
                t,
                x: r.x,
                y: r.y,
                busy: r.measuring.is_some(),
                finished: r.finished.take(),
                inbox: &inbox,
            };
            let action = self.brains[i].step(&obs);
            if r.measuring.is_none() {
                match action.cmd {
                    Command::Idle => {}
                    Command::Drive { x, y } => {
                        let (dx, dy) = (x - r.x, y - r.y);
                        let d = dx.hypot(dy);
                        if d <= step_len {
                            r.x = x;
                            r.y = y;
                        } else {
                            r.x += dx / d * step_len;
                            r.y += dy / d * step_len;
                        }
                    }
                    Command::Measure { point } => {
                        let (px, py) = self.grid.coords(point);
                        if (px - r.x).hypot(py - r.y) < 0.5 {
                            r.measuring = Some(point);
                            r.busy_until = t + self.p.measure_s;
                        }
                    }
                }
            }
            if let Some(msg) = action.broadcast {
                self.msgs += 1;
                self.bytes += msg.bytes();
                outgoing.push((i, Rc::new(msg)));
            }
        }

        // Radio: one hop, within range, with random loss.
        let range2 = self.p.radio_range_m * self.p.radio_range_m;
        for (from, msg) in &outgoing {
            let (fx, fy) = (self.rovers[*from].x, self.rovers[*from].y);
            for j in 0..self.rovers.len() {
                if j == *from || !self.rovers[j].alive {
                    continue;
                }
                let (dx, dy) = (self.rovers[j].x - fx, self.rovers[j].y - fy);
                if dx * dx + dy * dy > range2 {
                    continue;
                }
                if self.p.radio_loss > 0.0 && self.rng.f64() < self.p.radio_loss {
                    continue;
                }
                self.rovers[j].inbox.push(msg.clone());
            }
        }

        self.t += dt;

        // Stop: deadline, everyone dead, or everyone has had nothing to do for
        // longer than a lease (so that claims of failed rovers have run out).
        let mut any_alive = false;
        let mut all_resting = true;
        for (r, b) in self.rovers.iter().zip(self.brains.iter()) {
            if r.alive {
                any_alive = true;
                if r.measuring.is_some() || !b.is_resting() {
                    all_resting = false;
                }
            }
        }
        if all_resting {
            let since = *self.resting_since.get_or_insert(self.t);
            if self.t - since > self.p.lease_s + 2.0 * self.p.gossip_period_s {
                self.finished = true;
            }
        } else {
            self.resting_since = None;
        }
        if !any_alive || self.t >= self.p.deadline_h * 3600.0 {
            self.finished = true;
        }
        self.finished
    }

    pub fn run_to_end(&mut self) -> Metrics {
        while !self.step() {}
        self.metrics()
    }

    pub fn metrics(&self) -> Metrics {
        let n = self.grid.len() as u32;
        let n_miss = n - self.measured;
        Metrics {
            t_s: self.t,
            n_points: n,
            measured: self.measured,
            n_miss,
            n_dup: self.dups,
            j: self.p.c_miss * n_miss as f64 + self.p.c_dup * self.dups as f64,
            t_last_measure_s: self.t_last_measure,
            alive: self.rovers.iter().filter(|r| r.alive).count() as u32,
            msgs: self.msgs,
            bytes: self.bytes,
            finished: self.finished,
        }
    }

    /// Pairs of live rovers that can hear each other right now.
    pub fn links(&self) -> Vec<(u16, u16)> {
        let range2 = self.p.radio_range_m * self.p.radio_range_m;
        let mut out = Vec::new();
        for i in 0..self.rovers.len() {
            for j in i + 1..self.rovers.len() {
                let (a, b) = (&self.rovers[i], &self.rovers[j]);
                if a.alive && b.alive {
                    let (dx, dy) = (a.x - b.x, a.y - b.y);
                    if dx * dx + dy * dy <= range2 {
                        out.push((i as u16, j as u16));
                    }
                }
            }
        }
        out
    }
}
