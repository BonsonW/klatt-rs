use std::f32::consts::PI;

pub struct BandpassBiquad {
    b0: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
    last_f:  f32,
    last_bw: f32,
}

impl BandpassBiquad {
    pub fn new() -> Self {
        let mut bp = Self {
            b0: 0.0, b2: 0.0, a1: 0.0, a2: 0.0,
            x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
            last_f: -1.0, last_bw: -1.0,
        };
        bp.set_freq(500.0, 80.0, 22050.0);
        bp
    }

    pub fn set_freq(&mut self, f: f32, bw: f32, sr: f32) {
        if (f - self.last_f).abs() < 0.5 && (bw - self.last_bw).abs() < 0.5 {
            return;
        }
        self.last_f  = f;
        self.last_bw = bw;

        let f    = f.max(40.0).min(sr * 0.45);
        let bw   = bw.max(20.0);
        let w0   = 2.0 * PI * f / sr;
        let cos  = w0.cos();
        let sin  = w0.sin();
        let q    = f / bw;
        let alph = sin / (2.0 * q);
        let a0   = 1.0 + alph;

        self.b0 =  alph / a0;
        self.b2 = -alph / a0;
        self.a1 = -2.0 * cos / a0;
        self.a2 = (1.0 - alph) / a0;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b2 * self.x2
              - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// Rosenberg derivative glottal pulse.  `phase` is in [0, 1).
#[inline]
pub fn glottal_pulse(phase: f32, effort: f32) -> f32 {
    const NORM: f32 = 0.1;
    let tp = 0.5 - effort * 0.2;
    let tn = 0.25 - effort * 0.17;
    if phase < tp {
        NORM * 0.5 * (PI / tp) * (PI * phase / tp).sin()
    } else if phase < tp + tn {
        let p = phase - tp;
        -NORM * (PI / (2.0 * tn)) * (PI * p / (2.0 * tn)).sin()
    } else {
        0.0
    }
}

pub struct Xorshift32(u32);

impl Xorshift32 {
    pub fn new(seed: u32) -> Self { Self(seed.max(1)) }

    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as i32) as f32 / 2_147_483_648.0
    }
}

#[inline]
pub fn soft_clip(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.85 {
        return x;
    }
    let sign   = if x < 0.0 { -1.0f32 } else { 1.0 };
    let excess = a - 0.85;
    sign * (0.85 + 0.15 * excess / (excess + 1.0))
}
