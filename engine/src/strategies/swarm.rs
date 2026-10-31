//! The simplest swarm rule: greedy nearest point with a lease.
//!
//! Every rover keeps its own picture of the grid: which points it knows are
//! measured, and which are claimed by whom and until when. When it has nothing
//! to do, it picks the nearest point that is neither measured nor validly
//! claimed by someone else, claims it and tells its neighbours. A claim must be
//! renewed; if the owner fails, the claim simply runs out and the point becomes
//! free again. Nobody has to notice the failure.
//!
//! News spreads by flooding: whatever a rover learns for the first time, it
//! passes on in its next message. Every `gossip_period_s` it also sends its
//! whole "measured" bitmap, so that islands of the network that drifted apart
//! catch up when they meet again.
//!
//! Where it goes wrong: rovers in different islands cannot see each other's
//! work, so they may measure the same point twice.

use std::rc::Rc;

use crate::brain::{Action, Brain, Claim, Command, Grid, Message, Mission, Observation};

const NONE: u16 = u16::MAX;

pub struct Swarm {
    m: Mission,
    grid: Grid,
    done: Vec<u64>,
    done_count: u32,
    owner: Vec<u16>,
    since: Vec<f64>,
    until: Vec<f64>,
    target: Option<u32>,
    last_renew: f64,
    last_full: f64,
    out_done: Vec<u32>,
    out_claims: Vec<Claim>,
    resting: bool,
    /// When nothing was free, the earliest moment a blocking claim runs out.
    /// Until then searching again is pointless: only an expiry frees a point.
    next_search: f64,
}

impl Swarm {
    pub fn new(m: &Mission) -> Swarm {
        let grid = Grid::new(m.area_m, m.spacing_m);
        let n = grid.len();
        Swarm {
            m: m.clone(),
            grid,
            done: vec![0; (n + 63) / 64],
            done_count: 0,
            owner: vec![NONE; n],
            since: vec![0.0; n],
            until: vec![0.0; n],
            target: None,
            last_renew: 0.0,
            // Spread the full-map broadcasts of different rovers in time.
            last_full: -(m.id as f64 % m.gossip_period_s.max(1.0)),
            out_done: Vec::new(),
            out_claims: Vec::new(),
            resting: false,
            next_search: 0.0,
        }
    }

    fn is_done(&self, p: u32) -> bool {
        self.done[(p / 64) as usize] >> (p % 64) & 1 == 1
    }

    /// Returns true if the point was not known as measured before.
    fn set_done(&mut self, p: u32) -> bool {
        let w = &mut self.done[(p / 64) as usize];
        let bit = 1u64 << (p % 64);
        if *w & bit != 0 {
            return false;
        }
        *w |= bit;
        self.done_count += 1;
        self.owner[p as usize] = NONE;
        true
    }

    fn held_by_other(&self, p: u32, t: f64) -> bool {
        let o = self.owner[p as usize];
        o != NONE && o != self.m.id && self.until[p as usize] > t
    }

    /// Does the incoming claim beat the one we currently know for its point?
    fn accepts(&self, c: &Claim, t: f64) -> bool {
        let i = c.point as usize;
        let o = self.owner[i];
        if o == NONE || self.until[i] <= t {
            return true;
        }
        if o == c.owner {
            return c.until > self.until[i];
        }
        c.since < self.since[i] || (c.since == self.since[i] && c.owner < o)
    }

    fn merge_full(&mut self, full: &[u64]) {
        for (i, w) in full.iter().enumerate() {
            let new = *w & !self.done[i];
            if new != 0 {
                self.done[i] |= new;
                self.done_count += new.count_ones();
                let mut bits = new;
                while bits != 0 {
                    let b = bits.trailing_zeros();
                    self.owner[i * 64 + b as usize] = NONE;
                    bits &= bits - 1;
                }
            }
        }
    }

    /// Nearest free point, searching square rings outwards from (x, y).
    /// If there is none, also returns when the earliest blocking claim expires.
    fn nearest_free(&self, x: f64, y: f64, t: f64) -> (Option<u32>, f64) {
        if self.done_count as usize == self.grid.len() {
            return (None, f64::INFINITY);
        }
        let mut earliest = f64::INFINITY;
        let (cx, cy) = self.grid.cell(x, y);
        let side = self.grid.side as i64;
        let mut best: Option<(f64, u32)> = None;
        for r in 0..side {
            if let Some((d, _)) = best {
                if (r as f64 - 1.0) * self.grid.spacing > d {
                    break;
                }
            }
            let mut visit = |ix: i64, iy: i64| {
                if ix < 0 || iy < 0 || ix >= side || iy >= side {
                    return;
                }
                let p = self.grid.id(ix as u32, iy as u32);
                if self.is_done(p) {
                    return;
                }
                if self.held_by_other(p, t) {
                    earliest = earliest.min(self.until[p as usize]);
                    return;
                }
                let (px, py) = self.grid.coords(p);
                let d = (px - x).hypot(py - y);
                // Ties are broken by a per-rover hash, so that the swarm has no
                // built-in preferred direction.
                if best.map_or(true, |(bd, bp)| d < bd || (d == bd && self.tie(p) < self.tie(bp))) {
                    best = Some((d, p));
                }
            };
            if r == 0 {
                visit(cx, cy);
                continue;
            }
            for k in -r..=r {
                visit(cx + k, cy - r);
                visit(cx + k, cy + r);
            }
            for k in -r + 1..r {
                visit(cx - r, cy + k);
                visit(cx + r, cy + k);
            }
        }
        (best.map(|(_, p)| p), earliest)
    }

    fn tie(&self, p: u32) -> u64 {
        let mut z = (p as u64) << 16 | self.m.id as u64;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn claim(&mut self, p: u32, t: f64, since: f64) {
        let i = p as usize;
        self.owner[i] = self.m.id;
        self.since[i] = since;
        self.until[i] = t + self.m.lease_s;
        self.out_claims.push(Claim { point: p, owner: self.m.id, since, until: self.until[i] });
        self.last_renew = t;
    }
}

impl Brain for Swarm {
    fn step(&mut self, obs: &Observation) -> Action {
        let t = obs.t;

        // 1. Listen.
        for msg in obs.inbox {
            for &p in &msg.done {
                if self.set_done(p) {
                    self.out_done.push(p);
                }
            }
            if let Some(full) = &msg.full {
                if msg.full_count != self.done_count || self.done_count == 0 {
                    self.merge_full(full);
                }
            }
            for c in &msg.claims {
                if c.owner == self.m.id || self.is_done(c.point) {
                    continue;
                }
                if !self.accepts(c, t) {
                    // Someone is after our point with a weaker claim: remind them.
                    if self.target == Some(c.point) && self.owner[c.point as usize] == self.m.id {
                        let i = c.point as usize;
                        self.out_claims.push(Claim {
                            point: c.point,
                            owner: self.m.id,
                            since: self.since[i],
                            until: self.until[i],
                        });
                    }
                    continue;
                }
                let i = c.point as usize;
                self.owner[i] = c.owner;
                self.since[i] = c.since;
                self.until[i] = c.until;
                self.out_claims.push(*c);
            }
        }

        // 2. Own measurement finished.
        if let Some(p) = obs.finished {
            if self.set_done(p) {
                self.out_done.push(p);
            }
            self.target = None;
            self.next_search = t;
        }

        // 3. Decide.
        let cmd = if obs.busy {
            Command::Idle
        } else {
            if let Some(p) = self.target {
                if self.is_done(p) || self.held_by_other(p, t) {
                    self.target = None;
                    self.next_search = t;
                }
            }
            if self.target.is_none() && t >= self.next_search {
                let (found, earliest) = self.nearest_free(obs.x, obs.y, t);
                self.target = found;
                match found {
                    Some(p) => self.claim(p, t, t),
                    None => self.next_search = earliest,
                }
            }
            self.resting = self.target.is_none();
            match self.target {
                None => Command::Idle,
                Some(p) => {
                    let (tx, ty) = self.grid.coords(p);
                    if (tx - obs.x).hypot(ty - obs.y) < 0.25 {
                        Command::Measure { point: p }
                    } else {
                        Command::Drive { x: tx, y: ty }
                    }
                }
            }
        };

        // 4. Keep the claim alive (also while measuring).
        if let Some(p) = self.target {
            if t - self.last_renew >= self.m.lease_s / 2.0 {
                let since = self.since[p as usize];
                self.claim(p, t, since);
            }
        }

        // 5. Talk.
        let send_full = t - self.last_full >= self.m.gossip_period_s;
        let broadcast = if send_full || !self.out_done.is_empty() || !self.out_claims.is_empty() {
            let mut msg = Message {
                from: self.m.id,
                done: std::mem::take(&mut self.out_done),
                claims: std::mem::take(&mut self.out_claims),
                full: None,
                full_count: self.done_count,
            };
            if send_full {
                self.last_full = t;
                if self.done_count > 0 {
                    msg.full = Some(Rc::new(self.done.clone()));
                }
            }
            Some(msg)
        } else {
            None
        };

        Action { cmd, broadcast }
    }

    fn is_resting(&self) -> bool {
        self.resting
    }
}
