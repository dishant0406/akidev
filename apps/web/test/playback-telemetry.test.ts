import assert from "node:assert/strict";
import test from "node:test";
import { PlaybackTelemetry, type PlaybackSample } from "../src/lib/playback-telemetry.ts";
const sample = (now: number, position: number, extra: Partial<PlaybackSample> = {}): PlaybackSample => ({ now, position, paused: false, seeking: false, waiting: false, ended: false, bufferedLead: 10, ...extra });
test("first picture, active time, stalls and recovery are bounded and terminal is idempotent", () => {
  const t = new PlaybackTelemetry(0);
  assert.equal(t.frame(100), true); assert.equal(t.frame(200), false);
  t.sample(sample(1000, 0)); t.sample(sample(2000, 1));
  t.sample(sample(3000, 1, { waiting: true })); t.sample(sample(4000, 1, { waiting: true }));
  t.sample(sample(5000, 2));
  const result = t.finish("ended")!;
  assert.equal(result.first_frame_ms, 100); assert.equal(result.active_play_ms, 1000);
  assert.equal(result.stall_count, 1); assert.equal(result.stall_ms, 2000); assert.equal(result.recovery_count, 1);
  assert.equal(result.interval_rate_min, 0); assert.equal(result.actual_play_advance_seconds, 1);
  assert.equal(t.finish("failed"), null);
  assert.equal(Object.keys(result).some(k => /title|url|history|id$/.test(k)), false);
});
test("startup, pauses, seeks, discontinuities and suspended clocks do not count as stalls/play", () => {
  const t = new PlaybackTelemetry(0);
  t.sample(sample(1000, 0, { waiting: true })); t.sample(sample(2000, 0, { waiting: true }));
  t.frame(2500);
  t.sample(sample(3000, 0, { paused: true, waiting: true }));
  t.sample(sample(4000, 80, { seeking: true, waiting: true }));
  t.sample(sample(100000, 90));
  t.sample(sample(101000, 900, { waiting: true }));
  const result = t.finish("left")!;
  assert.equal(result.active_play_ms, 0); assert.equal(result.stall_count, 0); assert.equal(result.interval_samples, 0);
});
test("invalid samples and backwards clocks cannot poison totals", () => {
  const t = new PlaybackTelemetry(0); t.frame(100);
  t.sample(sample(NaN, 0)); t.sample(sample(1000, Infinity)); t.sample(sample(1000, 0));
  t.sample(sample(900, 2)); t.sample(sample(2000, 1, { bufferedLead: Infinity }));
  const result = t.finish("failed")!;
  assert.equal(result.interval_samples, 1); assert.equal(result.buffered_lead_seconds, 0);
  assert.equal(result.active_play_ms, 1000);
});
