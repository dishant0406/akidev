/** Fixed-size, session-local reducer. Never stores media identity or sample history. */
export type PlaybackSample = { now: number; position: number; paused: boolean; seeking: boolean; waiting: boolean; ended: boolean; bufferedLead: number };
export type PlaybackOutcome = "left" | "ended" | "failed";
const MAX_MS = 24 * 60 * 60 * 1000;
const bounded = (n: number, max: number) => Number.isFinite(n) ? Math.max(0, Math.min(n, max)) : 0;

export class PlaybackTelemetry {
  private last: PlaybackSample | null = null;
  private first: number | null = null;
  private active = 0;
  private advance = 0;
  private stalled = 0;
  private stalls = 0;
  private recoveries = 0;
  private inStall = false;
  private rates = 0;
  private rateTotal = 0;
  private rateMin = Infinity;
  private lead = 0;
  private leadTotal = 0;
  private leads = 0;
  private terminal = false;
  private readonly opened: number;
  constructor(opened: number) { this.opened = opened; }

  frame(now: number): boolean {
    if (this.terminal || this.first !== null || !Number.isFinite(now) || now < this.opened) return false;
    this.first = bounded(now - this.opened, MAX_MS);
    return true;
  }

  sample(input: PlaybackSample): void {
    if (this.terminal || !Number.isFinite(input.now) || !Number.isFinite(input.position) || input.position < 0) return;
    if (this.last && input.now <= this.last.now) return;
    const s = { ...input, bufferedLead: bounded(input.bufferedLead, 3600) };
    const previous = this.last;
    this.last = s;
    const eligible = this.first !== null && !s.paused && !s.seeking && !s.ended;
    const dt = previous ? s.now - previous.now : 0;
    const delta = previous ? s.position - previous.position : 0;
    // Ignore long scheduler suspensions and discontinuities (seeks), not just negative jumps.
    const interval = dt > 0 && dt <= 5000 && previous && !previous.paused && !previous.seeking && !previous.ended && delta >= 0 && delta <= dt / 1000 * 4 + 0.25;
    const stall = eligible && s.waiting && !!interval;
    if (stall && !this.inStall) this.stalls = Math.min(this.stalls + 1, 100000);
    if (this.inStall && eligible && !s.waiting && delta > 0 && interval) this.recoveries = Math.min(this.recoveries + 1, 100000);
    if (interval && eligible) {
if (this.inStall) this.stalled = bounded(this.stalled + dt, MAX_MS);
      if (!previous.waiting && !s.waiting && delta > 0) {
        this.active = bounded(this.active + dt, MAX_MS);
        this.advance = bounded(this.advance + delta, MAX_MS / 1000 * 4);
      }
      // Include zero-advance intervals: a mean excluding stalls would hide sustained-play trouble.
      const rate = bounded(delta / (dt / 1000), 4);
      if (this.rates < 100000) { this.rates++; this.rateTotal += rate; }
      this.rateMin = Math.min(this.rateMin, rate);
      if (this.leads < 100000) { this.leads++; this.leadTotal += s.bufferedLead; }
      this.lead = s.bufferedLead;
    }
    // `playing` can precede the next advancing frame. Keep the recovery pending until advance.
    this.inStall = stall || (this.inStall && eligible && !!interval && !s.waiting && delta === 0);
  }

  finish(outcome: PlaybackOutcome): Record<string, number | string | null> | null {
    if (this.terminal) return null;
    this.terminal = true;
    return {
      outcome, first_frame_ms: this.first, active_play_ms: Math.round(this.active),
      actual_play_advance_seconds: Math.round(this.advance * 1000) / 1000,
      stall_count: this.stalls, stall_ms: Math.round(this.stalled), recovery_count: this.recoveries,
      buffered_lead_seconds: this.lead, buffered_lead_mean_seconds: this.leads ? this.leadTotal / this.leads : 0,
      interval_rate_mean: this.rates ? this.rateTotal / this.rates : 0,
      interval_rate_min: this.rates ? this.rateMin : 0, interval_samples: this.rates,
    };
  }
}
