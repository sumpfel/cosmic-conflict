//! Deterministic, platform-independent random numbers and noise.
//!
//! The star system is generated on the server *and* on every client from the same seed, so it
//! must produce bit-identical results everywhere. Rules: only `+ - * /`, `sqrt`, `floor` and
//! `libm` functions — never `f32::sin`/`exp`/`powf` (they call the platform libm).

/// SplitMix64: tiny, fast and stable forever (we own the implementation).
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    /// Derives an independent stream for a sub-system (e.g. "deposits").
    pub fn fork(&self, salt: u64) -> Rng {
        let mut r = Rng(self.0 ^ salt.wrapping_mul(0xD6E8_FEB8_6659_FD93));
        r.next_u64();
        r
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    /// Uniform integer in [lo, hi).
    pub fn range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo) as u64) as u32
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    /// Angle in [0, 2π).
    pub fn angle(&mut self) -> f32 {
        self.f32() * core::f32::consts::TAU
    }
}

#[inline]
fn hash3(seed: u32, x: i32, y: i32, z: i32) -> u32 {
    let mut h = seed
        ^ (x as u32).wrapping_mul(0x27d4_eb2d)
        ^ (y as u32).wrapping_mul(0x1656_67b1)
        ^ (z as u32).wrapping_mul(0x9e37_79b1);
    h = (h ^ (h >> 15)).wrapping_mul(0x85eb_ca6b);
    h = (h ^ (h >> 13)).wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}

/// The 12 cube-edge gradients of classic Perlin noise (+4 repeats for a power of two).
const GRADS: [(f32, f32, f32); 16] = [
    (1.0, 1.0, 0.0),
    (-1.0, 1.0, 0.0),
    (1.0, -1.0, 0.0),
    (-1.0, -1.0, 0.0),
    (1.0, 0.0, 1.0),
    (-1.0, 0.0, 1.0),
    (1.0, 0.0, -1.0),
    (-1.0, 0.0, -1.0),
    (0.0, 1.0, 1.0),
    (0.0, -1.0, 1.0),
    (0.0, 1.0, -1.0),
    (0.0, -1.0, -1.0),
    (1.0, 1.0, 0.0),
    (-1.0, 1.0, 0.0),
    (0.0, -1.0, 1.0),
    (0.0, -1.0, -1.0),
];

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// 3D gradient (Perlin) noise — sampled on the unit sphere it gives seamless planet terrain.
/// Output roughly in [-1, 1].
#[derive(Clone, Copy, Debug)]
pub struct Noise {
    seed: u32,
}

impl Noise {
    pub fn new(seed: u64) -> Self {
        Noise { seed: (seed ^ (seed >> 32)) as u32 }
    }

    pub fn get(&self, x: f32, y: f32, z: f32) -> f32 {
        let (x0, y0, z0) = (x.floor(), y.floor(), z.floor());
        let (ix, iy, iz) = (x0 as i32, y0 as i32, z0 as i32);
        let (fx, fy, fz) = (x - x0, y - y0, z - z0);
        let g = |gx: i32, gy: i32, gz: i32, dx: f32, dy: f32, dz: f32| {
            let (a, b, c) = GRADS[(hash3(self.seed, gx, gy, gz) & 15) as usize];
            a * dx + b * dy + c * dz
        };
        let (u, v, w) = (fade(fx), fade(fy), fade(fz));
        let x00 = lerp(g(ix, iy, iz, fx, fy, fz), g(ix + 1, iy, iz, fx - 1.0, fy, fz), u);
        let x10 = lerp(g(ix, iy + 1, iz, fx, fy - 1.0, fz), g(ix + 1, iy + 1, iz, fx - 1.0, fy - 1.0, fz), u);
        let x01 = lerp(g(ix, iy, iz + 1, fx, fy, fz - 1.0), g(ix + 1, iy, iz + 1, fx - 1.0, fy, fz - 1.0), u);
        let x11 = lerp(
            g(ix, iy + 1, iz + 1, fx, fy - 1.0, fz - 1.0),
            g(ix + 1, iy + 1, iz + 1, fx - 1.0, fy - 1.0, fz - 1.0),
            u,
        );
        lerp(lerp(x00, x10, v), lerp(x01, x11, v), w)
    }

    /// Fractal Brownian motion, normalised to roughly [-1, 1].
    pub fn fbm(&self, x: f32, y: f32, z: f32, octaves: u32) -> f32 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for o in 0..octaves {
            let off = o as f32 * 17.13;
            sum += amp * self.get(x * freq + off, y * freq - off, z * freq + off * 0.5);
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }

    /// Ridged multifractal noise in roughly [0, 1] (sharp mountain ridges).
    pub fn ridged(&self, x: f32, y: f32, z: f32, octaves: u32) -> f32 {
        let (mut sum, mut amp, mut freq, mut weight, mut norm) = (0.0, 0.5, 1.0, 1.0, 0.0);
        for o in 0..octaves {
            let off = o as f32 * 31.7;
            let mut n = 1.0 - self.get(x * freq + off, y * freq + off, z * freq - off).abs();
            n *= n;
            n *= weight;
            weight = (n * 2.0).clamp(0.0, 1.0);
            sum += n * amp;
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_stable() {
        let mut r = Rng::new(42);
        let a = r.next_u64();
        let mut r2 = Rng::new(42);
        assert_eq!(r2.next_u64(), a);
        for _ in 0..1000 {
            let f = r.f32();
            assert!((0.0..1.0).contains(&f));
        }
    }

    #[test]
    fn noise_range() {
        let n = Noise::new(7);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..60 {
            for j in 0..60 {
                let v = n.fbm(i as f32 * 0.137, j as f32 * 0.141, (i + j) as f32 * 0.05, 5);
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        assert!(lo > -1.5 && hi < 1.5, "{lo} {hi}");
        assert!(hi - lo > 0.5);
    }
}
