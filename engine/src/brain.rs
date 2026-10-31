//! The contract between the world and a rover brain.
//!
//! The world knows the truth: true positions, who has failed, which radio
//! messages arrive, how many times each grid point was measured. A brain is the
//! software of ONE rover. It only gets what a real rover could sense
//! (`Observation`) and acts only through commands and radio messages
//! (`Action`). It never learns directly that another rover has failed: it can
//! only notice that it no longer hears from it.
//!
//! Simplifications of this first version, to be removed in later steps:
//! - the rover knows its own position exactly (no drift, no GPS needed);
//! - the terrain is flat and fully passable;
//! - energy is unlimited, the only limit is the mission deadline.

use std::rc::Rc;

/// What every brain is told before launch (uploaded on Earth).
#[derive(Clone, Debug)]
pub struct Mission {
    pub id: u16,
    pub rovers: usize,
    pub area_m: f64,
    pub spacing_m: f64,
    pub speed_mps: f64,
    pub measure_s: f64,
    pub lease_s: f64,
    pub gossip_period_s: f64,
}

/// A claim: "I am going to measure this point". Valid until `until`, unless
/// the owner renews it. If two rovers claim the same point, the earlier claim
/// wins (ties: lower id).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Claim {
    pub point: u32,
    pub owner: u16,
    pub since: f64,
    pub until: f64,
}

/// A radio message. The world delivers it to every live rover within radio
/// range (one hop). Anything beyond one hop must be relayed by the brains.
#[derive(Clone, Debug, Default)]
pub struct Message {
    pub from: u16,
    /// Points the sender has learned are measured, not yet passed on.
    pub done: Vec<u32>,
    /// Claims the sender is announcing or passing on.
    pub claims: Vec<Claim>,
    /// Occasionally the whole "measured" bitmap, so that islands of the
    /// network that meet again can catch up.
    pub full: Option<Rc<Vec<u64>>>,
    pub full_count: u32,
}

impl Message {
    /// Approximate size on the air, for the statistics.
    pub fn bytes(&self) -> u64 {
        let full = self.full.as_ref().map_or(0, |f| f.len() * 8);
        (8 + self.done.len() * 4 + self.claims.len() * 22 + full) as u64
    }
}

/// What a brain senses in one time step.
pub struct Observation<'a> {
    /// Mission time, s.
    pub t: f64,
    /// Own position relative to the lander, m (exact in this version).
    pub x: f64,
    pub y: f64,
    /// A measurement is still running; commands are ignored until it ends.
    pub busy: bool,
    /// The measurement of this grid point has just finished.
    pub finished: Option<u32>,
    /// Radio messages received since the last step.
    pub inbox: &'a [Rc<Message>],
}

pub enum Command {
    Idle,
    /// Drive straight towards (x, y) at full speed.
    Drive { x: f64, y: f64 },
    /// Start measuring the given grid point. Accepted only on the spot.
    Measure { point: u32 },
}

pub struct Action {
    pub cmd: Command,
    pub broadcast: Option<Message>,
}

pub trait Brain {
    fn step(&mut self, obs: &Observation) -> Action;
    /// Test harness only, a real rover does not need it: true if the brain
    /// believes there is nothing left for it to do. Used to stop the run.
    fn is_resting(&self) -> bool;
}

/// The measurement grid. Point id = iy * side + ix; the lander is at (0, 0),
/// the centre of the square.
#[derive(Clone, Copy, Debug)]
pub struct Grid {
    pub side: u32,
    pub spacing: f64,
    pub half: f64,
}

impl Grid {
    pub fn new(area_m: f64, spacing_m: f64) -> Grid {
        let side = (area_m / spacing_m).round() as u32 + 1;
        Grid { side, spacing: spacing_m, half: (side - 1) as f64 * spacing_m / 2.0 }
    }

    pub fn len(&self) -> usize {
        (self.side * self.side) as usize
    }

    pub fn id(&self, ix: u32, iy: u32) -> u32 {
        iy * self.side + ix
    }

    pub fn coords(&self, p: u32) -> (f64, f64) {
        let ix = p % self.side;
        let iy = p / self.side;
        (ix as f64 * self.spacing - self.half, iy as f64 * self.spacing - self.half)
    }

    /// Nearest grid cell to a position, clamped to the area.
    pub fn cell(&self, x: f64, y: f64) -> (i64, i64) {
        let max = self.side as i64 - 1;
        let ix = ((x + self.half) / self.spacing).round() as i64;
        let iy = ((y + self.half) / self.spacing).round() as i64;
        (ix.clamp(0, max), iy.clamp(0, max))
    }
}
