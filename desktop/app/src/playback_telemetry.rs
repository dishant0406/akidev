//! Fixed-space, private session measurements; clocks and samples are supplied for pure tests.
#[derive(Default)]
pub struct Session {
    pub first_frame_ms: Option<u64>,
    pub active_ms: u64,
    pub advance_seconds: f64,
    pub stall_count: u64,
    pub stall_ms: u64,
    pub recoveries: u64,
    pub samples: u64,
    pub rate_total: f64,
    pub rate_min: Option<f64>,
    pub lead_total: f64,
    pub lead: f64,
    last: Option<Sample>,
    stalled: bool,
    terminal: bool,
}
#[derive(Clone, Copy)]
pub struct Sample {
    pub ms: u64,
    pub position: f64,
    pub paused: bool,
    pub seeking: bool,
    pub buffering: bool,
    pub ended: bool,
    pub lead: f64,
}
const MAX_MS: u64 = 86_400_000;
impl Session {
    pub fn frame(&mut self, ms: u64) -> bool {
        if self.terminal || self.first_frame_ms.is_some() {
            return false;
        }
        self.first_frame_ms = Some(ms.min(MAX_MS));
        true
    }
    pub fn reset_interval(&mut self) {
        self.last = None;
        self.stalled = false;
    }
    pub fn sample(&mut self, s: Sample) {
        if self.terminal || !s.position.is_finite() || s.position < 0.0 {
            return;
        }
        if self.last.is_some_and(|last| s.ms <= last.ms) {
            return;
        }
        let previous = self.last.replace(s);
        let eligible = self.first_frame_ms.is_some() && !s.paused && !s.seeking && !s.ended;
        let interval = previous.filter(|p| {
            let dt = s.ms.saturating_sub(p.ms);
            let delta = s.position - p.position;
            dt > 0
                && dt <= 5000
                && !p.paused
                && !p.seeking
                && !p.ended
                && delta >= 0.0
                && delta <= dt as f64 / 1000.0 * 4.0 + 0.25
        });
        let stall = eligible && s.buffering && interval.is_some();
        if stall && !self.stalled {
            self.stall_count = (self.stall_count + 1).min(100_000);
        }
        if let Some(p) = interval.filter(|_| eligible) {
            let dt = s.ms - p.ms;
            let delta = s.position - p.position;
            if self.stalled && !s.buffering && delta > 0.0 {
                self.recoveries = (self.recoveries + 1).min(100_000);
            }
            if self.stalled {
                self.stall_ms = (self.stall_ms + dt).min(MAX_MS);
            }
            if !p.buffering && !s.buffering && delta > 0.0 {
                self.active_ms = (self.active_ms + dt).min(MAX_MS);
                self.advance_seconds = (self.advance_seconds + delta).min(345_600.0);
            }
            let rate = (delta / (dt as f64 / 1000.0)).clamp(0.0, 4.0);
            self.rate_min = Some(self.rate_min.map_or(rate, |min| min.min(rate)));
            self.lead = if s.lead.is_finite() {
                s.lead.clamp(0.0, 3600.0)
            } else {
                0.0
            };
            if self.samples < 100_000 {
                self.samples += 1;
                self.rate_total += rate;
                self.lead_total += self.lead;
            }
        }
        self.stalled = stall
            || (self.stalled
                && eligible
                && interval.is_some()
                && !s.buffering
                && previous.is_some_and(|p| s.position == p.position));
    }
    pub fn finish(&mut self) -> bool {
        if self.terminal {
            return false;
        }
        self.terminal = true;
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample(ms: u64, position: f64) -> Sample {
        Sample {
            ms,
            position,
            paused: false,
            seeking: false,
            buffering: false,
            ended: false,
            lead: 12.0,
        }
    }
    #[test]
    fn active_stalls_recovery_and_terminal_are_session_scoped() {
        let mut t = Session::default();
        assert!(t.frame(100));
        assert!(!t.frame(200));
        t.sample(sample(1000, 0.0));
        t.sample(sample(2000, 1.0));
        t.sample(Sample {
            buffering: true,
            ..sample(3000, 1.0)
        });
        t.sample(Sample {
            buffering: true,
            ..sample(4000, 1.0)
        });
        t.sample(sample(5000, 2.0));
        assert_eq!(
            (t.active_ms, t.stall_count, t.stall_ms, t.recoveries),
            (1000, 1, 2000, 1)
        );
        assert_eq!(t.rate_min, Some(0.0));
        assert!(t.finish());
        assert!(!t.finish());
        t.sample(sample(6000, 3.0));
        assert_eq!(t.active_ms, 1000);
    }
    #[test]
    fn load_pause_seek_and_scheduler_gaps_are_not_play_or_stalls() {
        let mut t = Session::default();
        t.sample(Sample {
            buffering: true,
            ..sample(1000, 0.0)
        });
        t.sample(Sample {
            buffering: true,
            ..sample(2000, 0.0)
        });
        assert_eq!(t.stall_count, 0);
        t.frame(2500);
        t.sample(Sample {
            paused: true,
            buffering: true,
            ..sample(3000, 0.0)
        });
        t.sample(Sample {
            seeking: true,
            buffering: true,
            ..sample(4000, 80.0)
        });
        t.sample(sample(100_000, 90.0));
        assert_eq!((t.active_ms, t.stall_count, t.samples), (0, 0, 0));
    }
}
