//! Baseline: fixed stripes (boustrophedon).
//!
//! Before launch every rover is given a band of grid columns and walks it in a
//! serpentine order. It needs no radio at all, so it produces no duplicates,
//! but if a rover fails, the rest of its band stays unmeasured: nobody knows
//! about it and nobody would take it over.

use crate::brain::{Action, Brain, Command, Grid, Mission, Observation};

pub struct Stripes {
    grid: Grid,
    plan: Vec<u32>,
    next: usize,
}

impl Stripes {
    pub fn new(m: &Mission) -> Stripes {
        let grid = Grid::new(m.area_m, m.spacing_m);
        let side = grid.side as usize;
        let n = m.rovers.max(1);
        let id = m.id as usize;
        let a = id * side / n;
        let b = (id + 1) * side / n;
        let mut plan = Vec::with_capacity((b - a) * side);
        for (j, ix) in (a..b).enumerate() {
            for k in 0..side {
                let iy = if j % 2 == 0 { k } else { side - 1 - k };
                plan.push(grid.id(ix as u32, iy as u32));
            }
        }
        // Start from the end of the band that is closer to the lander.
        if let (Some(&first), Some(&last)) = (plan.first(), plan.last()) {
            let d = |p: u32| {
                let (x, y) = grid.coords(p);
                x * x + y * y
            };
            if d(last) < d(first) {
                plan.reverse();
            }
        }
        Stripes { grid, plan, next: 0 }
    }
}

impl Brain for Stripes {
    fn step(&mut self, obs: &Observation) -> Action {
        if obs.finished.is_some() {
            self.next += 1;
        }
        let cmd = if obs.busy || self.next >= self.plan.len() {
            Command::Idle
        } else {
            let p = self.plan[self.next];
            let (tx, ty) = self.grid.coords(p);
            if (tx - obs.x).hypot(ty - obs.y) < 0.25 {
                Command::Measure { point: p }
            } else {
                Command::Drive { x: tx, y: ty }
            }
        };
        Action { cmd, broadcast: None }
    }

    fn is_resting(&self) -> bool {
        self.next >= self.plan.len()
    }
}
