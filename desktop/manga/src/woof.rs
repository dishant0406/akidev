//! Underdog Woof: a small English speech model that runs on the device (https://underdog.ai/woof, CC BY-NC 4.0: free for
//! non-commercial use, and the app must say "Voice by Underdog Woof").
//!
//! This is a port of the model's own numpy runtime (`underdog_woof/runtime.py`), checked against it on the same text. The
//! weights are one file, `woof_int8.npz`: a zip of numpy arrays plus `__tree__`, a JSON text that says how they nest.
//!
//! Speech is made in two steps. The speech model reads the voice, then the text, then writes 25 frames of 32 numbers for
//! every second of speech, one at a time, each drawn with a little noise. The decoder turns those frames into 24 kHz audio.

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

pub const SAMPLE_RATE: u32 = 24_000;
/// The model's two voices.
pub const VOICES: [&str; 2] = ["juno", "mira"];

const EPS: f32 = f32::EPSILON;
const TEMPERATURE: f32 = 0.5;
const EOS_THRESHOLD: f32 = 0.9;
const MAX_SPEECH_POSITIONS: usize = 1100;
/// A piece of text is read in one go: longer sentences are cut at clauses, as the Python runtime does.
const CLAUSE_MAX: usize = 120;
const RETRIES: usize = 3;
/// The decoder's steps from this one on work at the highest rates, so they run a few frames at a time.
const CHUNKED_FROM: usize = 6;
const CHUNK_FRAMES: usize = 8;
const SAMPLES_PER_FRAME: usize = 960;

const CHARS: &str =
    " abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,!?;:'\"-()&/%$€£@#+=*_<>[]…—";

// ---------------------------------------------------------------------------------------------------------- numpy files

enum Data {
    Float(Vec<f32>),
    Int8(Vec<i8>),
}

struct Array {
    shape: Vec<usize>,
    data: Data,
}

impl Array {
    fn floats(&self) -> Result<&[f32]> {
        match &self.data {
            Data::Float(values) => Ok(values),
            Data::Int8(_) => bail!("expected numbers with a fraction"),
        }
    }

    fn ints(&self) -> Result<&[i8]> {
        match &self.data {
            Data::Int8(values) => Ok(values),
            Data::Float(_) => bail!("expected whole numbers"),
        }
    }
}

fn half_to_float(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = ((bits >> 10) & 0x1f) as i32;
    let fraction = (bits & 0x3ff) as f32;
    match exponent {
        0 => sign * fraction * 2f32.powi(-24),
        31 if fraction == 0.0 => sign * f32::INFINITY,
        31 => f32::NAN,
        _ => sign * (1.0 + fraction / 1024.0) * 2f32.powi(exponent - 15),
    }
}

/// Reads one `.npy` file: the header says the type and shape, the numbers follow.
fn parse_npy(bytes: &[u8]) -> Result<(Vec<usize>, String, &[u8])> {
    ensure!(
        bytes.len() > 12 && &bytes[..6] == b"\x93NUMPY",
        "not a numpy file"
    );
    let (length, start) = if bytes[6] == 1 {
        (u16::from_le_bytes([bytes[8], bytes[9]]) as usize, 10)
    } else {
        (
            u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize,
            12,
        )
    };
    let header = std::str::from_utf8(bytes.get(start..start + length).context("short header")?)?;
    let kind = header
        .split("'descr':")
        .nth(1)
        .and_then(|rest| rest.split('\'').nth(1))
        .context("no type in the header")?
        .to_string();
    ensure!(
        header.contains("'fortran_order': False"),
        "column-major arrays are not supported"
    );
    let shape_text = header
        .split("'shape':")
        .nth(1)
        .and_then(|rest| rest.split(['(', ')']).nth(1))
        .context("no shape in the header")?;
    let shape = shape_text
        .split(',')
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim().parse::<usize>())
        .collect::<Result<Vec<_>, _>>()?;
    Ok((shape, kind, &bytes[start + length..]))
}

/// The arrays of a weights file, by name, and the tree that says how they nest.
struct Weights {
    arrays: HashMap<String, Array>,
    tree: Value,
}

fn load_weights(path: &Path) -> Result<Weights> {
    let mut zip =
        zip::ZipArchive::new(std::fs::File::open(path).context("cannot open the weights")?)?;
    let mut arrays = HashMap::new();
    let mut tree = None;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        let name = entry.name()?.trim_end_matches(".npy").to_string();
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes)?;
        let (shape, kind, body) = parse_npy(&bytes)?;
        if name == "__tree__" {
            // A text: UTF-32, four bytes a letter.
            let text: String = body
                .chunks_exact(4)
                .filter_map(|c| char::from_u32(u32::from_le_bytes([c[0], c[1], c[2], c[3]])))
                .collect();
            tree = Some(serde_json::from_str(&text).context("the weights' tree is not JSON")?);
            continue;
        }
        let data = match kind.as_str() {
            "<f4" => Data::Float(
                body.chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect(),
            ),
            "<f2" => Data::Float(
                body.chunks_exact(2)
                    .map(|c| half_to_float(u16::from_le_bytes([c[0], c[1]])))
                    .collect(),
            ),
            "|i1" => Data::Int8(body.iter().map(|&b| b as i8).collect()),
            other => bail!("{name}: the type {other} is not supported"),
        };
        arrays.insert(name, Array { shape, data });
    }
    Ok(Weights {
        arrays,
        tree: tree.context("the weights have no tree")?,
    })
}

impl Weights {
    /// The array a tree node points to: `{"@": "<name>"}`.
    fn array(&self, node: &Value) -> Result<&Array> {
        let name = node
            .get("@")
            .and_then(Value::as_str)
            .context("a node does not point to an array")?;
        self.arrays
            .get(name)
            .with_context(|| format!("{name} is missing"))
    }

    /// A tensor as fp32 with its shape: a plain array, or one stored as int8 in groups (`q8`, `s`, `shape`, `axis`, `g`).
    fn tensor(&self, node: &Value) -> Result<(Vec<f32>, Vec<usize>)> {
        let Some(q8) = node.get("q8") else {
            let array = self.array(node)?;
            return Ok((array.floats()?.to_vec(), array.shape.clone()));
        };
        let q = self.array(q8)?;
        let scales = self.array(&node["s"])?;
        let shape: Vec<usize> = serde_json::from_value(node["shape"].clone())?;
        let axis = node["axis"].as_u64().context("no axis")? as usize;
        let group = node["g"].as_u64().context("no group size")? as usize;
        ensure!(
            axis <= 1 && shape.len() <= 3 && axis < shape.len(),
            "an unexpected tensor layout"
        );
        let (rows, columns) = (q.shape[0], q.shape[1]);
        let (q, s) = (q.ints()?, scales.floats()?);
        let groups = scales.shape[1];
        let mut flat = vec![0f32; rows * columns];
        for row in 0..rows {
            for column in 0..columns {
                flat[row * columns + column] =
                    q[row * columns + column] as f32 * s[row * groups + column / group];
            }
        }
        if axis == 0 {
            return Ok((flat, shape));
        }
        // The groups are stored with the axis first; put it back in second place: (rows, a, b) -> (a, rows, b).
        let (a, b) = (shape[0], shape[2]);
        let mut out = vec![0f32; flat.len()];
        for row in 0..rows {
            for i in 0..a {
                for j in 0..b {
                    out[(i * rows + row) * b + j] = flat[row * (a * b) + i * b + j];
                }
            }
        }
        Ok((out, shape))
    }

    fn floats(&self, node: &Value) -> Result<Vec<f32>> {
        Ok(self.tensor(node)?.0)
    }

    /// A linear layer: int8 rows with a scale each (`q`, `s`) or plain (`w`), and an optional bias `b`.
    fn linear(&self, node: &Value) -> Result<Linear> {
        let (weight, shape) = if node.get("q").is_some() {
            let q = self.array(&node["q"])?;
            let s = self.array(&node["s"])?.floats()?;
            let (rows, columns) = (q.shape[0], q.shape[1]);
            let q = q.ints()?;
            let mut weight = vec![0f32; rows * columns];
            for row in 0..rows {
                for column in 0..columns {
                    weight[row * columns + column] = q[row * columns + column] as f32 * s[row];
                }
            }
            (weight, vec![rows, columns])
        } else {
            let array = self.array(&node["w"])?;
            (array.floats()?.to_vec(), array.shape.clone())
        };
        let bias = match node.get("b") {
            Some(b) => self.array(b)?.floats()?.to_vec(),
            None => vec![0.0; shape[0]],
        };
        Ok(Linear {
            weight,
            rows: shape[0],
            columns: shape[1],
            bias,
        })
    }
}

// ---------------------------------------------------------------------------------------------------------- arithmetic

fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut sums = [0f32; 8];
    let (a_chunks, b_chunks) = (a.chunks_exact(8), b.chunks_exact(8));
    let (a_rest, b_rest) = (a_chunks.remainder(), b_chunks.remainder());
    for (x, y) in a_chunks.zip(b_chunks) {
        for i in 0..8 {
            sums[i] += x[i] * y[i];
        }
    }
    sums.iter().sum::<f32>() + a_rest.iter().zip(b_rest).map(|(x, y)| x * y).sum::<f32>()
}

fn silu(x: f32) -> f32 {
    x / (1.0 + (-x).exp())
}

fn gelu(x: f32) -> f32 {
    0.5 * x * (1.0 + erf(x * std::f32::consts::FRAC_1_SQRT_2))
}

fn elu(x: f32) -> f32 {
    if x > 0.0 { x } else { x.exp_m1() }
}

/// Abramowitz and Stegun 7.1.26, error under 1.5e-7 (the standard library has no erf).
fn erf(x: f32) -> f32 {
    let a = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * a);
    let y = 1.0
        - (((((1.061_405_4 * t - 1.453_152_1) * t) + 1.421_413_8) * t - 0.284_496_72) * t
            + 0.254_829_6)
            * t
            * (-a * a).exp();
    y.copysign(x)
}

fn layer_norm(x: &[f32]) -> Vec<f32> {
    let mean = x.iter().sum::<f32>() / x.len() as f32;
    let variance = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / x.len() as f32;
    let scale = 1.0 / (variance + 1e-5).sqrt();
    x.iter().map(|v| (v - mean) * scale).collect()
}

/// 1 / the root mean square, which the model's norm layers scale by.
fn inverse_rms(x: &[f32]) -> f32 {
    1.0 / (dot(x, x) / x.len() as f32 + EPS).sqrt()
}

struct Linear {
    /// `rows` x `columns`, one output per row.
    weight: Vec<f32>,
    rows: usize,
    columns: usize,
    bias: Vec<f32>,
}

impl Linear {
    fn apply(&self, x: &[f32]) -> Vec<f32> {
        (0..self.rows)
            .map(|row| {
                dot(
                    &self.weight[row * self.columns..(row + 1) * self.columns],
                    x,
                ) + self.bias[row]
            })
            .collect()
    }
}

/// Softmax over a row, in place.
fn softmax(row: &mut [f32]) {
    let max = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut total = 0.0;
    for v in row.iter_mut() {
        *v = (*v - max).exp();
        total += *v;
    }
    for v in row.iter_mut() {
        *v /= total;
    }
}

/// Turns a pair (first half, second half) into (-second, first): the half-turn of rotary position.
fn rotate_half(x: &[f32], out: &mut [f32]) {
    let half = x.len() / 2;
    for i in 0..half {
        out[i] = -x[half + i];
        out[half + i] = x[i];
    }
}

// ---------------------------------------------------------------------------------------------------------- noise

/// The noise the model is trained with is PyTorch's: the same numbers for the same seed. A Mersenne Twister, and a
/// Box-Muller turn of 24-bit draws, 16 at a time.
struct Noise {
    state: [u32; 624],
    next: usize,
}

impl Noise {
    fn new(seed: u32) -> Noise {
        let mut state = [0u32; 624];
        state[0] = seed;
        for i in 1..624 {
            state[i] = 1_812_433_253u32
                .wrapping_mul(state[i - 1] ^ (state[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Noise { state, next: 624 }
    }

    fn draw(&mut self) -> u32 {
        if self.next >= 624 {
            for i in 0..624 {
                let y = (self.state[i] & 0x8000_0000) | (self.state[(i + 1) % 624] & 0x7fff_ffff);
                let mut value = self.state[(i + 397) % 624] ^ (y >> 1);
                if y & 1 != 0 {
                    value ^= 0x9908_b0df;
                }
                self.state[i] = value;
            }
            self.next = 0;
        }
        let mut y = self.state[self.next];
        self.next += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// `n` numbers from the normal distribution (`n` a multiple of 16).
    fn normal(&mut self, n: usize) -> Vec<f32> {
        let u: Vec<f64> = (0..n)
            .map(|_| (self.draw() & 0xff_ffff) as f64 * 2f64.powi(-24))
            .collect();
        let mut out = vec![0f32; n];
        for block in 0..n / 16 {
            for j in 0..8 {
                let radius = (-2.0 * (1.0 - u[block * 16 + j]).ln()).sqrt();
                let angle = 2.0 * std::f64::consts::PI * u[block * 16 + 8 + j];
                out[block * 16 + j] = (radius * angle.cos()) as f32;
                out[block * 16 + 8 + j] = (radius * angle.sin()) as f32;
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------- speech model

struct Layer {
    qkv: Linear,
    out: Linear,
    up: Linear,
    down: Linear,
}

struct Speech {
    layers: Vec<Layer>,
    heads: usize,
    head_size: usize,
    ff: usize,
    hw: usize,
    latent: usize,
    depth: usize,
    /// `[cos, cos]` and `[-sin, sin]` for every position.
    cos: Vec<f32>,
    sin: Vec<f32>,
    condition: Linear,
    ada: Linear,
    head_blocks: Vec<(Linear, Linear)>,
    latent_in: Linear,
    head_in: Linear,
    head_out: Linear,
    text_embedding: Vec<f32>,
    start: Vec<f32>,
}

/// What each layer remembers of the positions already read: keys and values, per head.
struct Memory {
    keys: Vec<f32>,
    values: Vec<f32>,
    head_size: usize,
}

impl Memory {
    fn at(&self, head: usize, position: usize) -> usize {
        (head * MAX_SPEECH_POSITIONS + position) * self.head_size
    }
}

impl Speech {
    fn load(weights: &Weights) -> Result<Speech> {
        let node = &weights.tree["speech"];
        let config = &node["cfg"];
        let number = |name: &str| {
            config[name]
                .as_u64()
                .map(|n| n as usize)
                .context("a number is missing")
        };
        let (heads, d, ff) = (number("H")?, number("d")?, number("ff")?);
        let head_size = d / heads;
        let layers = node["layers"]
            .as_array()
            .context("no layers")?
            .iter()
            .map(|layer| {
                let parts = layer.as_array().context("a layer is not a list")?;
                ensure!(parts.len() == 4, "a layer has four parts");
                Ok(Layer {
                    qkv: weights.linear(&parts[0])?,
                    out: weights.linear(&parts[1])?,
                    up: weights.linear(&parts[2])?,
                    down: weights.linear(&parts[3])?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            layers.len() == number("L")?,
            "the layer count differs from the config"
        );
        let base = config["rope_base"].as_f64().context("no rope base")? as f32;
        let (mut cos, mut sin) = (
            vec![0f32; MAX_SPEECH_POSITIONS * head_size],
            vec![0f32; MAX_SPEECH_POSITIONS * head_size],
        );
        for position in 0..MAX_SPEECH_POSITIONS {
            for i in 0..head_size / 2 {
                let frequency = 1.0 / base.powf((2 * i) as f32 / head_size as f32);
                let angle = position as f32 * frequency;
                let at = position * head_size;
                cos[at + i] = angle.cos();
                cos[at + head_size / 2 + i] = angle.cos();
                sin[at + i] = -angle.sin();
                sin[at + head_size / 2 + i] = angle.sin();
            }
        }
        let head_blocks = node["hblocks"]
            .as_array()
            .context("no head blocks")?
            .iter()
            .map(|block| {
                let parts = block.as_array().context("a head block is not a list")?;
                ensure!(parts.len() == 2, "a head block has two parts");
                Ok((weights.linear(&parts[0])?, weights.linear(&parts[1])?))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Speech {
            layers,
            heads,
            head_size,
            ff,
            hw: number("hw")?,
            latent: number("latent")?,
            depth: number("depth")?,
            cos,
            sin,
            condition: weights.linear(&node["ce"])?,
            ada: weights.linear(&node["ada"])?,
            head_blocks,
            latent_in: weights.linear(&node["lat_in"])?,
            head_in: weights.linear(&node["h_inp"])?,
            head_out: weights.linear(&node["h_out"])?,
            text_embedding: weights.floats(&node["text_emb"])?,
            start: weights.floats(&node["bos"])?,
        })
    }

    fn rope(&self, x: &mut [f32], position: usize) {
        let size = self.head_size;
        let mut turned = vec![0f32; size];
        let half = size / 2;
        for head in x.chunks_exact_mut(size) {
            // The two halves swap places; the sign is already in the sine table.
            turned[..half].copy_from_slice(&head[half..]);
            turned[half..].copy_from_slice(&head[..half]);
            for i in 0..size {
                head[i] = head[i] * self.cos[position * size + i]
                    + turned[i] * self.sin[position * size + i];
            }
        }
    }

    /// Reads all of the voice and the text at once, filling each layer's memory. Returns the last position's vector.
    fn read(&self, mut x: Vec<Vec<f32>>, memory: &mut [Memory]) -> Vec<f32> {
        let (heads, size) = (self.heads, self.head_size);
        let n = x.len();
        for (layer, memory) in self.layers.iter().zip(memory.iter_mut()) {
            let mut queries = vec![vec![0f32; heads * size]; n];
            for (position, row) in x.iter().enumerate() {
                let scale = inverse_rms(row);
                let mut qkv: Vec<f32> = layer.qkv.apply(row).iter().map(|v| v * scale).collect();
                let (q, rest) = qkv.split_at_mut(heads * size);
                let (k, v) = rest.split_at_mut(heads * size);
                self.rope(q, position);
                self.rope(k, position);
                for head in 0..heads {
                    let at = memory.at(head, position);
                    memory.keys[at..at + size].copy_from_slice(&k[head * size..(head + 1) * size]);
                    memory.values[at..at + size]
                        .copy_from_slice(&v[head * size..(head + 1) * size]);
                }
                queries[position].copy_from_slice(q);
            }
            for position in 0..n {
                let attended = self.attend(memory, &queries[position], position);
                let added = layer.out.apply(&attended);
                for (a, b) in x[position].iter_mut().zip(added) {
                    *a += b;
                }
                x[position] = self.feed_forward(layer, &x[position]);
            }
        }
        x.pop().unwrap_or_default()
    }

    /// One position looks back at positions 0..=position and mixes their values.
    fn attend(&self, memory: &Memory, query: &[f32], position: usize) -> Vec<f32> {
        let size = self.head_size;
        let scale = (size as f32).powf(-0.5);
        let mut out = vec![0f32; self.heads * size];
        for head in 0..self.heads {
            let q = &query[head * size..(head + 1) * size];
            let mut scores: Vec<f32> = (0..=position)
                .map(|p| {
                    let at = memory.at(head, p);
                    dot(q, &memory.keys[at..at + size]) * scale
                })
                .collect();
            softmax(&mut scores);
            let target = &mut out[head * size..(head + 1) * size];
            for (p, weight) in scores.iter().enumerate() {
                let at = memory.at(head, p);
                for (t, v) in target.iter_mut().zip(&memory.values[at..at + size]) {
                    *t += weight * v;
                }
            }
        }
        out
    }

    fn feed_forward(&self, layer: &Layer, x: &[f32]) -> Vec<f32> {
        let scale = inverse_rms(x);
        let up: Vec<f32> = layer.up.apply(x).iter().map(|v| v * scale).collect();
        let mixed: Vec<f32> = (0..self.ff)
            .map(|i| silu(up[i]) * up[self.ff + i])
            .collect();
        let down = layer.down.apply(&mixed);
        x.iter().zip(down).map(|(a, b)| a + b).collect()
    }

    /// One new position: its vector goes through every layer, adding to the layers' memory.
    fn step(&self, mut x: Vec<f32>, position: usize, memory: &mut [Memory]) -> Vec<f32> {
        let (heads, size) = (self.heads, self.head_size);
        for (layer, memory) in self.layers.iter().zip(memory.iter_mut()) {
            let scale = inverse_rms(&x);
            let mut qkv: Vec<f32> = layer.qkv.apply(&x).iter().map(|v| v * scale).collect();
            let (q, rest) = qkv.split_at_mut(heads * size);
            let (k, v) = rest.split_at_mut(heads * size);
            self.rope(q, position);
            self.rope(k, position);
            for head in 0..heads {
                let at = memory.at(head, position);
                memory.keys[at..at + size].copy_from_slice(&k[head * size..(head + 1) * size]);
                memory.values[at..at + size].copy_from_slice(&v[head * size..(head + 1) * size]);
            }
            let attended = self.attend(memory, q, position);
            for (a, b) in x.iter_mut().zip(layer.out.apply(&attended)) {
                *a += b;
            }
            x = self.feed_forward(layer, &x);
        }
        x
    }

    /// The conditioning the head uses, and the score for "the speech ends here".
    fn conditioning(&self, x: &[f32]) -> (Vec<f32>, f32) {
        let scale = inverse_rms(x);
        let out: Vec<f32> = self.condition.apply(x);
        // The bias is added after the scale: apply() added it before, so take it off and put it back.
        let scaled: Vec<f32> = out
            .iter()
            .zip(&self.condition.bias)
            .map(|(v, b)| (v - b) * scale + b)
            .collect();
        (
            scaled[..self.hw].iter().map(|&v| silu(v)).collect(),
            scaled[self.hw],
        )
    }

    /// The flow head: from the conditioning and the noise, the correction the noise needs.
    fn head(&self, s: &[f32], z: &[f32]) -> Vec<f32> {
        let hw = self.hw;
        let a = self.ada.apply(s);
        let mut h = self.head_in.apply(z);
        for (j, (first, second)) in self.head_blocks.iter().enumerate() {
            let o = 3 * hw * j;
            let normal = layer_norm(&h);
            let y: Vec<f32> = (0..hw)
                .map(|i| a[o + i] + normal[i] * a[o + hw + i])
                .collect();
            let hidden: Vec<f32> = first.apply(&y).into_iter().map(silu).collect();
            let out = second.apply(&hidden);
            for i in 0..hw {
                h[i] += a[o + 2 * hw + i] * out[i];
            }
        }
        let o = 3 * hw * self.depth;
        let normal = layer_norm(&h);
        let y: Vec<f32> = (0..hw)
            .map(|i| a[o + i] + normal[i] * a[o + hw + i])
            .collect();
        self.head_out.apply(&y)
    }

    /// The frames of speech for `text` in `voice` (16 x d numbers): `latent` numbers a frame.
    fn generate(
        &self,
        voice: &[f32],
        text: &[usize],
        max_frames: usize,
        noise: &mut Noise,
    ) -> Vec<f32> {
        let d = self.heads * self.head_size;
        let mut x: Vec<Vec<f32>> = voice.chunks_exact(d).map(<[f32]>::to_vec).collect();
        for &id in text {
            x.push(self.text_embedding[id * d..(id + 1) * d].to_vec());
        }
        x.push(self.start.clone());
        let first = x.len();
        let max_frames = max_frames.min(MAX_SPEECH_POSITIONS - first);
        let mut memory: Vec<Memory> = self
            .layers
            .iter()
            .map(|_| Memory {
                keys: vec![0.0; self.heads * MAX_SPEECH_POSITIONS * self.head_size],
                values: vec![0.0; self.heads * MAX_SPEECH_POSITIONS * self.head_size],
                head_size: self.head_size,
            })
            .collect();
        let last = self.read(x, &mut memory);
        let (mut s, mut end) = self.conditioning(&last);
        let end_threshold = (EOS_THRESHOLD / (1.0 - EOS_THRESHOLD)).ln();
        let mut frames = Vec::new();
        for frame in 0..max_frames {
            if frame > 3 && end > end_threshold {
                break;
            }
            let sample: Vec<f32> = noise
                .normal(self.latent)
                .iter()
                .map(|v| v * TEMPERATURE)
                .collect();
            let correction = self.head(&s, &sample);
            let z: Vec<f32> = sample.iter().zip(correction).map(|(a, b)| a + b).collect();
            let next = self.latent_in.apply(&z);
            frames.extend_from_slice(&z);
            let out = self.step(next, first + frame, &mut memory);
            (s, end) = self.conditioning(&out);
        }
        frames
    }
}

// ---------------------------------------------------------------------------------------------------------- decoder

/// A causal convolution over time: each output looks at this and the `taps - 1` frames before it.
struct Conv {
    /// One (inputs x outputs) matrix per tap.
    taps: Vec<Vec<f32>>,
    inputs: usize,
    outputs: usize,
    bias: Vec<f32>,
}

/// A convolution that makes `stride` samples from each frame, each frame's tail adding to the next frame's start.
struct ConvUp {
    /// inputs x (stride * outputs): what a frame makes of its own samples, and what it leaves for the next frame.
    own: Vec<f32>,
    tail: Vec<f32>,
    inputs: usize,
    outputs: usize,
    stride: usize,
    /// The bias, repeated `stride` times.
    bias: Vec<f32>,
}

enum Step {
    Elu,
    Conv(Conv),
    ConvUp(ConvUp),
    Residual(Conv, Conv),
}

struct Block {
    norm1: (Vec<f32>, Vec<f32>),
    norm2: (Vec<f32>, Vec<f32>),
    qkv: Vec<f32>,
    out: Vec<f32>,
    up: Vec<f32>,
    down: Vec<f32>,
}

struct Decoder {
    input: Vec<f32>,
    input_bias: Vec<f32>,
    blocks: Vec<Block>,
    steps: Vec<Step>,
    heads: usize,
    head_size: usize,
    window: usize,
    eps: f32,
    inverse_frequency: Vec<f32>,
}

/// `out[t] += x[t] (inputs) times weight (inputs x outputs)`, for rows of `x`.
fn multiply(x: &[f32], inputs: usize, weight: &[f32], outputs: usize, out: &mut [f32]) {
    for (row, target) in x.chunks_exact(inputs).zip(out.chunks_exact_mut(outputs)) {
        for (i, &value) in row.iter().enumerate() {
            if value == 0.0 {
                continue;
            }
            for (t, w) in target
                .iter_mut()
                .zip(&weight[i * outputs..(i + 1) * outputs])
            {
                *t += value * w;
            }
        }
    }
}

impl Conv {
    fn load(weights: &Weights, node: &Value) -> Result<Conv> {
        let (w, shape) = weights.tensor(&node["w"])?;
        let (outputs, inputs, taps) = (shape[0], shape[1], shape[2]);
        let taps = (0..taps)
            .map(|j| {
                let mut tap = vec![0f32; inputs * outputs];
                for o in 0..outputs {
                    for i in 0..inputs {
                        tap[i * outputs + o] = w[(o * inputs + i) * shape[2] + j];
                    }
                }
                tap
            })
            .collect();
        Ok(Conv {
            taps,
            inputs,
            outputs,
            bias: weights.floats(&node["b"])?,
        })
    }

    /// `x` is (time x inputs); `residual` (time x outputs) is added to the result. `before` holds the last `taps - 1`
    /// inputs of the chunk before (zeros at the start) and is given the last ones of this chunk.
    fn apply(&self, x: &[f32], before: &mut Vec<f32>, residual: Option<&[f32]>) -> Vec<f32> {
        let time = x.len() / self.inputs;
        let mut padded = std::mem::take(before);
        padded.extend_from_slice(x);
        let mut out = vec![0f32; time * self.outputs];
        for (t, target) in out.chunks_exact_mut(self.outputs).enumerate() {
            target.copy_from_slice(&self.bias);
            if let Some(residual) = residual {
                for (a, b) in target
                    .iter_mut()
                    .zip(&residual[t * self.outputs..(t + 1) * self.outputs])
                {
                    *a += b;
                }
            }
        }
        // Output t uses inputs t to t + taps - 1 of the padded ones, tap j on input t + j.
        for (j, tap) in self.taps.iter().enumerate() {
            multiply(
                &padded[j * self.inputs..(j + time) * self.inputs],
                self.inputs,
                tap,
                self.outputs,
                &mut out,
            );
        }
        *before = padded.split_off(time * self.inputs);
        out
    }
}

impl ConvUp {
    fn load(weights: &Weights, node: &Value) -> Result<ConvUp> {
        let (w, shape) = weights.tensor(&node["w"])?;
        let (inputs, outputs, taps) = (shape[0], shape[1], shape[2]);
        let stride = node["stride"].as_u64().context("no stride")? as usize;
        ensure!(taps == 2 * stride, "a kernel that is not twice the stride");
        let block = stride * outputs;
        let (mut own, mut tail) = (vec![0f32; inputs * block], vec![0f32; inputs * block]);
        for i in 0..inputs {
            for o in 0..outputs {
                for j in 0..stride {
                    own[i * block + j * outputs + o] = w[(i * outputs + o) * taps + j];
                    tail[i * block + j * outputs + o] = w[(i * outputs + o) * taps + stride + j];
                }
            }
        }
        let bias = weights.floats(&node["b"])?;
        Ok(ConvUp {
            own,
            tail,
            inputs,
            outputs,
            stride,
            bias: bias.repeat(stride),
        })
    }

    /// `carry` is what the last frame of the chunk before left for this chunk's first frame (zeros at the start).
    fn apply(&self, x: &[f32], carry: &mut Vec<f32>) -> Vec<f32> {
        let time = x.len() / self.inputs;
        let block = self.stride * self.outputs;
        let mut out = vec![0f32; time * block];
        for row in out.chunks_exact_mut(block) {
            row.copy_from_slice(&self.bias);
        }
        multiply(x, self.inputs, &self.own, block, &mut out);
        // Each frame's tail goes to the start of the next frame.
        multiply(
            &x[..(time - 1) * self.inputs],
            self.inputs,
            &self.tail,
            block,
            &mut out[block..],
        );
        for (a, b) in out[..block].iter_mut().zip(carry.iter()) {
            *a += b;
        }
        let mut left = vec![0f32; block];
        multiply(
            &x[(time - 1) * self.inputs..],
            self.inputs,
            &self.tail,
            block,
            &mut left,
        );
        *carry = left;
        out
    }
}

fn apply_norm(x: &[f32], eps: f32, norm: &(Vec<f32>, Vec<f32>)) -> Vec<f32> {
    let mean = x.iter().sum::<f32>() / x.len() as f32;
    let variance = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / x.len() as f32;
    let scale = 1.0 / (variance + eps).sqrt();
    x.iter()
        .enumerate()
        .map(|(i, v)| (v - mean) * scale * norm.0[i] + norm.1[i])
        .collect()
}

impl Decoder {
    fn load(weights: &Weights) -> Result<Decoder> {
        let node = &weights.tree["decoder"];
        let config = &node["cfg"];
        let heads = config["heads"].as_u64().context("no heads")? as usize;
        let head_size = config["head_dim"].as_u64().context("no head size")? as usize;
        let inputs = node["inp"].as_array().context("no input layer")?;
        let blocks = node["tf"]
            .as_array()
            .context("no transformer")?
            .iter()
            .map(|block| {
                let pair = |name: &str| -> Result<(Vec<f32>, Vec<f32>)> {
                    let both = block[name].as_array().context("a norm is missing")?;
                    Ok((weights.floats(&both[0])?, weights.floats(&both[1])?))
                };
                Ok(Block {
                    norm1: pair("ln1")?,
                    norm2: pair("ln2")?,
                    qkv: weights.floats(&block["qkv"])?,
                    out: weights.floats(&block["o"])?,
                    up: weights.floats(&block["fc1"])?,
                    down: weights.floats(&block["fc2"])?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let steps = node["seanet"]
            .as_array()
            .context("no convolutions")?
            .iter()
            .map(|step| {
                let (kind, body) = (step[0].as_str().unwrap_or(""), &step[1]);
                Ok(match kind {
                    "elu" => Step::Elu,
                    "conv" => Step::Conv(Conv::load(weights, body)?),
                    "convt" => Step::ConvUp(ConvUp::load(weights, body)?),
                    "res" => {
                        let (first, second) = (
                            Conv::load(weights, &body[0])?,
                            Conv::load(weights, &body[1])?,
                        );
                        ensure!(
                            second.taps.len() == 1,
                            "a residual step's second convolution has one tap"
                        );
                        Step::Residual(first, second)
                    }
                    other => bail!("the step {other} is not known"),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Decoder {
            input: weights.floats(&inputs[0])?,
            input_bias: weights.floats(&inputs[1])?,
            blocks,
            steps,
            heads,
            head_size,
            window: config["window"].as_u64().context("no window")? as usize,
            eps: config["eps"].as_f64().context("no eps")? as f32,
            inverse_frequency: weights.floats(&node["inv_freq"])?,
        })
    }

    /// Frames that look at each other, within a window of the last `window` frames.
    fn transformer(&self, mut h: Vec<Vec<f32>>) -> Vec<Vec<f32>> {
        let (heads, size) = (self.heads, self.head_size);
        let d = heads * size;
        let time = h.len();
        let scale = 1.0 / (size as f32).sqrt();
        for block in &self.blocks {
            let mut qkv = Vec::with_capacity(time);
            for (position, row) in h.iter().enumerate() {
                let y = apply_norm(row, self.eps, &block.norm1);
                let mut out: Vec<f32> = (0..3 * d)
                    .map(|r| dot(&block.qkv[r * d..(r + 1) * d], &y))
                    .collect();
                // q and k are turned by their position, v is not.
                let mut turned = vec![0f32; size];
                for part in 0..2 {
                    for head in 0..heads {
                        let at = part * d + head * size;
                        rotate_half(&out[at..at + size], &mut turned);
                        for i in 0..size {
                            let angle = position as f32 * self.inverse_frequency[i % (size / 2)];
                            out[at + i] = out[at + i] * angle.cos() + turned[i] * angle.sin();
                        }
                    }
                }
                qkv.push(out);
            }
            for position in 0..time {
                let from = (position + 1).saturating_sub(self.window);
                let mut mixed = vec![0f32; d];
                for head in 0..heads {
                    let q = &qkv[position][head * size..(head + 1) * size];
                    let mut scores: Vec<f32> = (from..=position)
                        .map(|p| dot(q, &qkv[p][d + head * size..d + (head + 1) * size]) * scale)
                        .collect();
                    softmax(&mut scores);
                    for (p, weight) in (from..=position).zip(&scores) {
                        let v = &qkv[p][2 * d + head * size..2 * d + (head + 1) * size];
                        for (m, value) in mixed[head * size..(head + 1) * size].iter_mut().zip(v) {
                            *m += weight * value;
                        }
                    }
                }
                for (r, a) in h[position].iter_mut().enumerate() {
                    *a += dot(&block.out[r * d..(r + 1) * d], &mixed);
                }
                let y = apply_norm(&h[position], self.eps, &block.norm2);
                let hidden_size = block.up.len() / d;
                let hidden: Vec<f32> = (0..hidden_size)
                    .map(|r| gelu(dot(&block.up[r * d..(r + 1) * d], &y)))
                    .collect();
                for (r, a) in h[position].iter_mut().enumerate() {
                    *a += dot(&block.down[r * hidden_size..(r + 1) * hidden_size], &hidden);
                }
            }
        }
        h
    }

    /// Frames of speech (already scaled back) to audio samples.
    fn decode(&self, latent: &[f32], width: usize) -> Vec<f32> {
        let d = self.heads * self.head_size;
        let h: Vec<Vec<f32>> = latent
            .chunks_exact(width)
            .map(|frame| {
                (0..d)
                    .map(|r| {
                        dot(&self.input[r * width..(r + 1) * width], frame) + self.input_bias[r]
                    })
                    .collect()
            })
            .collect();
        let frames = h.len();
        let x: Vec<f32> = self.transformer(h).concat();
        let mut states = self.fresh_states();
        // The first steps work at low rates on the whole sentence; the rest make 24,000 samples a second, which for a
        // whole sentence would be hundreds of megabytes, so they run a few frames at a time.
        let x = self.run(0..CHUNKED_FROM, x, &mut states);
        let per_frame = x.len() / frames;
        let mut audio = Vec::with_capacity(frames * SAMPLES_PER_FRAME);
        for chunk in x.chunks(CHUNK_FRAMES * per_frame) {
            audio.extend(self.run(CHUNKED_FROM..self.steps.len(), chunk.to_vec(), &mut states));
        }
        audio
    }

    /// What each step carries from one chunk to the next, empty at the start.
    fn fresh_states(&self) -> Vec<Vec<f32>> {
        self.steps
            .iter()
            .map(|step| match step {
                Step::Elu => Vec::new(),
                Step::Conv(conv) | Step::Residual(conv, _) => {
                    vec![0.0; (conv.taps.len() - 1) * conv.inputs]
                }
                Step::ConvUp(up) => vec![0.0; up.stride * up.outputs],
            })
            .collect()
    }

    fn run(
        &self,
        steps: std::ops::Range<usize>,
        mut x: Vec<f32>,
        states: &mut [Vec<f32>],
    ) -> Vec<f32> {
        for index in steps {
            let state = &mut states[index];
            x = match &self.steps[index] {
                Step::Elu => {
                    x.iter_mut().for_each(|v| *v = elu(*v));
                    x
                }
                Step::Conv(conv) => conv.apply(&x, state, None),
                Step::ConvUp(up) => up.apply(&x, state),
                Step::Residual(first, second) => {
                    let hidden: Vec<f32> = x.iter().copied().map(elu).collect();
                    let mut hidden = first.apply(&hidden, state, None);
                    hidden.iter_mut().for_each(|v| *v = elu(*v));
                    second.apply(&hidden, &mut Vec::new(), Some(&x))
                }
            };
        }
        x
    }
}

// ---------------------------------------------------------------------------------------------------------- the speaker

/// Woof loaded and ready to speak.
pub struct Woof {
    speech: Speech,
    decoder: Decoder,
    mean: Vec<f32>,
    std: Vec<f32>,
    voices: HashMap<String, Vec<f32>>,
    max_frames: usize,
    noise: Mutex<Noise>,
}

fn encode(text: &str) -> Vec<usize> {
    let mut ids = vec![1];
    ids.extend(
        text.chars()
            .map(|c| match CHARS.chars().position(|known| known == c) {
                Some(at) => at + 3,
                None => 2,
            }),
    );
    ids
}

/// The longest stretch of quiet (seconds) between the first and the last loud part: a sign the speech stalled.
fn longest_quiet(audio: &[f32]) -> f32 {
    let hop = 960;
    let frames = audio.len() / hop;
    if frames < 3 {
        return 0.0;
    }
    let quiet: Vec<bool> = (0..frames)
        .map(|f| {
            let slice = &audio[f * hop..(f + 1) * hop];
            let rms = (slice.iter().map(|v| v * v).sum::<f32>() / hop as f32).sqrt();
            20.0 * (rms + 1e-9).log10() < -50.0
        })
        .collect();
    let (Some(first), Some(last)) = (
        quiet.iter().position(|&q| !q),
        quiet.iter().rposition(|&q| !q),
    ) else {
        return 0.0;
    };
    if last <= first {
        return 0.0;
    }
    let (mut best, mut run) = (0, 0);
    for &q in &quiet[first..=last] {
        run = if q { run + 1 } else { 0 };
        best = best.max(run);
    }
    best as f32 * hop as f32 / SAMPLE_RATE as f32
}

impl Woof {
    /// Loads `woof_int8.npz` from `folder`.
    pub fn open(folder: &Path) -> Result<Woof> {
        let weights = load_weights(&folder.join("woof_int8.npz"))?;
        let voices = weights.tree["voices"]
            .as_object()
            .context("the weights have no voices")?
            .iter()
            .map(|(name, node)| Ok((name.clone(), weights.floats(node)?)))
            .collect::<Result<HashMap<_, _>>>()?;
        Ok(Woof {
            speech: Speech::load(&weights)?,
            decoder: Decoder::load(&weights)?,
            mean: weights.floats(&weights.tree["mean"])?,
            std: weights.floats(&weights.tree["std"])?,
            voices,
            max_frames: weights.tree["max_frames"]
                .as_u64()
                .context("no frame limit")? as usize,
            noise: Mutex::new(Noise::new(0)),
        })
    }

    /// Starts the noise again from `seed`: the same text then gives the same speech.
    pub fn seed(&self, seed: u32) {
        *self.noise.lock().unwrap_or_else(|e| e.into_inner()) = Noise::new(seed);
    }

    /// The audio for one piece of text that is not too long: it is made again, up to three times, when it runs away or stalls.
    fn piece(&self, voice: &[f32], text: &str) -> Vec<f32> {
        let ids = encode(text);
        let width = self.speech.latent;
        let mut best: Option<((i32, f32, f32), Vec<f32>)> = None;
        for _ in 0..=RETRIES {
            let frames = {
                let mut noise = self.noise.lock().unwrap_or_else(|e| e.into_inner());
                self.speech
                    .generate(voice, &ids, self.max_frames, &mut noise)
            };
            let scaled: Vec<f32> = frames
                .chunks_exact(width)
                .flat_map(|frame| {
                    frame
                        .iter()
                        .enumerate()
                        .map(|(i, v)| v * self.std[i] + self.mean[i])
                })
                .collect();
            let audio = self.decoder.decode(&scaled, width);
            let quiet = longest_quiet(&audio);
            let seconds = audio.len() as f32 / SAMPLE_RATE as f32;
            let letters = text.chars().count().max(1) as f32;
            let per_char = seconds / letters;
            // At most 0.095 s a letter, but never under 2 s: a one-word heading is slower than that per letter and would be made four times.
            let ran_away =
                seconds > (0.095 * letters).max(2.0) || frames.len() / width >= self.max_frames;
            let bad = (quiet >= 1.0) as i32 + ran_away as i32;
            let score = (bad, quiet, per_char);
            if best.as_ref().is_none_or(|(old, _)| score < *old) {
                best = Some((score, audio));
            }
            if bad == 0 {
                break;
            }
        }
        best.map(|(_, audio)| audio).unwrap_or_default()
    }

    /// The speech for `text` in one of the model's voices, mono at 24 kHz.
    pub fn speak(&self, text: &str, voice: Option<&str>) -> Result<Vec<f32>> {
        let name = voice.unwrap_or(VOICES[0]);
        let voice = self
            .voices
            .get(name)
            .with_context(|| format!("Woof has no voice {name}"))?;
        let mut audio = Vec::new();
        for piece in split_text(&crate::woof_text::spoken(text)) {
            audio.extend(self.piece(voice, &piece));
        }
        Ok(audio)
    }
}

// ---------------------------------------------------------------------------------------------------------- long text

/// Cuts text into pieces the model reads in one go: sentences, and clauses where a sentence is longer than 120 letters.
fn split_text(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut chars = text.trim().chars().peekable();
    while let Some(c) = chars.next() {
        current.push(c);
        if matches!(c, '.' | '!' | '?' | '…') && chars.peek().is_some_and(|n| n.is_whitespace()) {
            sentences.push(std::mem::take(&mut current));
            while chars.peek().is_some_and(|n| n.is_whitespace()) {
                chars.next();
            }
        }
    }
    sentences.push(current);
    let mut pieces = Vec::new();
    for sentence in sentences.into_iter().filter(|s| !s.trim().is_empty()) {
        if sentence.chars().count() <= CLAUSE_MAX {
            pieces.push(sentence);
            continue;
        }
        let mut merged: Vec<String> = Vec::new();
        for clause in clauses(&sentence) {
            match merged.last_mut() {
                Some(last) if last.chars().count() + 1 + clause.chars().count() <= CLAUSE_MAX => {
                    last.push(' ');
                    last.push_str(&clause);
                }
                _ => merged.push(clause),
            }
        }
        pieces.extend(merged);
    }
    pieces
}

/// A sentence cut after each comma, semicolon or colon that is followed by a space.
fn clauses(sentence: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut current = String::new();
    let mut chars = sentence.chars().peekable();
    while let Some(c) = chars.next() {
        current.push(c);
        if matches!(c, ',' | ';' | ':') && chars.peek().is_some_and(|n| n.is_whitespace()) {
            found.push(std::mem::take(&mut current).trim().to_string());
            while chars.peek().is_some_and(|n| n.is_whitespace()) {
                chars.next();
            }
        }
    }
    if !current.trim().is_empty() {
        found.push(current.trim().to_string());
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_pytorchs() {
        // torch.manual_seed(0); torch.randn(16)[:3]
        let numbers = Noise::new(0).normal(16);
        assert!((numbers[0] + 1.1258398).abs() < 1e-5, "{}", numbers[0]);
        assert!((numbers[1] + 1.1523602).abs() < 1e-5, "{}", numbers[1]);
        assert!((numbers[2] + 0.2505786).abs() < 1e-5, "{}", numbers[2]);
    }

    #[test]
    fn half_floats_read() {
        assert_eq!(half_to_float(0x3c00), 1.0);
        assert_eq!(half_to_float(0xc000), -2.0);
        assert_eq!(half_to_float(0x3555), 0.333_251_95);
    }

    #[test]
    fn long_text_is_cut_at_sentences_and_clauses() {
        assert_eq!(
            split_text("Hello there. How are you?"),
            ["Hello there.", "How are you?"]
        );
        let long = format!("{}, {}.", "a".repeat(80), "b".repeat(80));
        assert_eq!(split_text(&long).len(), 2);
    }

    /// Runs the real model: `WOOF_DIR=<folder with woof_int8.npz> WOOF_OUT=<wav path> cargo test --release woof_speaks -- --ignored`.
    #[test]
    #[ignore]
    fn woof_speaks() {
        let folder = std::env::var("WOOF_DIR").expect("WOOF_DIR");
        let text = std::env::var("WOOF_TEXT")
            .unwrap_or_else(|_| "Thanks for calling. How can I help you today?".into());
        let woof = Woof::open(Path::new(&folder)).unwrap();
        let started = std::time::Instant::now();
        let voice = std::env::var("WOOF_VOICE").unwrap_or_else(|_| "juno".into());
        let audio = woof.speak(&text, Some(&voice)).unwrap();
        let seconds = audio.len() as f32 / SAMPLE_RATE as f32;
        eprintln!(
            "{seconds:.2} s of speech in {:.2} s",
            started.elapsed().as_secs_f32()
        );
        if let Ok(path) = std::env::var("WOOF_OUT") {
            let pcm: Vec<u8> = audio
                .iter()
                .flat_map(|v| ((v.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes())
                .collect();
            let mut wav = Vec::new();
            wav.extend_from_slice(b"RIFF");
            wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
            wav.extend_from_slice(b"WAVEfmt ");
            wav.extend_from_slice(&16u32.to_le_bytes());
            wav.extend_from_slice(&1u16.to_le_bytes());
            wav.extend_from_slice(&1u16.to_le_bytes());
            wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
            wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
            wav.extend_from_slice(&2u16.to_le_bytes());
            wav.extend_from_slice(&16u16.to_le_bytes());
            wav.extend_from_slice(b"data");
            wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
            wav.extend_from_slice(&pcm);
            std::fs::write(path, wav).unwrap();
        }
        assert!(seconds > 1.0);
    }
}
