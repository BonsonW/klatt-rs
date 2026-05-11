use crate::dsp::{glottal_pulse, soft_clip, BandpassBiquad, Xorshift32};
use crate::phonemes::FormantParams;

pub const SAMPLE_RATE: u32 = 22050;
const SR: f32 = SAMPLE_RATE as f32;
const EFFORT:         f32 = 0.5;
const GAIN:           f32 = 3.5;
const TRANSITION_MS:  f32 = 35.0;

pub struct SynthEvent {
    pub params:      FormantParams,
    pub dur_ms:      f32,
    pub pitch_start: f32,
    pub pitch_end:   f32,
}

// Internal per-sample state used for smooth interpolation across phoneme boundaries.
#[derive(Clone)]
struct Frame {
    voicing:    f32,
    f1: f32, bw1: f32, a1: f32,
    f2: f32, bw2: f32, a2: f32,
    f3: f32, bw3: f32, a3: f32,
    aspiration: f32,
}

impl Frame {
    fn silence() -> Self {
        Self { voicing: 0.0,
            f1: 500.0, bw1: 80.0,  a1: 0.0,
            f2: 1500.0, bw2: 120.0, a2: 0.0,
            f3: 2500.0, bw3: 160.0, a3: 0.0,
            aspiration: 0.0,
        }
    }

    fn from_params(p: &FormantParams) -> Self {
        Self {
            voicing:    p.voicing,
            f1: p.f1,  bw1: p.bw1,  a1: p.a1,
            f2: p.f2,  bw2: p.bw2,  a2: p.a2,
            f3: p.f3,  bw3: p.bw3,  a3: p.a3,
            aspiration: p.aspiration,
        }
    }

    fn lerp(&self, other: &Self, t: f32) -> Self {
        macro_rules! l { ($f:ident) => { self.$f + (other.$f - self.$f) * t }; }
        Self {
            voicing: l!(voicing),
            f1: l!(f1), bw1: l!(bw1), a1: l!(a1),
            f2: l!(f2), bw2: l!(bw2), a2: l!(a2),
            f3: l!(f3), bw3: l!(bw3), a3: l!(a3),
            aspiration: l!(aspiration),
        }
    }
}

/// Compute the `Frame` for phoneme `p` at normalised position `pos` [0..1].
fn frame_at(p: &FormantParams, pos: f32) -> Frame {
    if p.is_stop {
        return if pos < 0.40 {
            // Closure: silent
            Frame { voicing: 0.0, a1: 0.0, a2: 0.0, a3: 0.0, aspiration: 0.0,
                f1: p.f1, bw1: p.bw1, f2: p.f2, bw2: p.bw2, f3: p.f3, bw3: p.bw3 }
        } else if pos < 0.60 {
            // Burst: noise plosion
            Frame { voicing: 0.0, aspiration: 0.8,
                f1: p.f1, bw1: p.bw1, a1: p.a1,
                f2: p.f2, bw2: p.bw2, a2: p.a2,
                f3: p.f3, bw3: p.bw3, a3: p.a3 }
        } else {
            // Release: full params
            Frame::from_params(p)
        };
    }

    if let Some(glide) = p.glide_to {
        let mut fr = Frame::from_params(p);
        if pos > 0.25 && pos < 0.75 {
            let t = (pos - 0.25) / 0.50;
            fr.f1 = p.f1 + (glide[0] - p.f1) * t;
            fr.f2 = p.f2 + (glide[1] - p.f2) * t;
            fr.f3 = p.f3 + (glide[2] - p.f3) * t;
        } else if pos >= 0.75 {
            fr.f1 = glide[0]; fr.f2 = glide[1]; fr.f3 = glide[2];
        }
        return fr;
    }

    Frame::from_params(p)
}

pub fn synthesize(events: &[SynthEvent]) -> Vec<i16> {
    let trans_n = (TRANSITION_MS * SR / 1000.0) as usize;

    let mut samples = Vec::new();
    let mut bp = [BandpassBiquad::new(), BandpassBiquad::new(), BandpassBiquad::new()];
    let mut rng = Xorshift32::new(0x12345678);
    let mut phase: f32 = 0.0;

    let mut prev = Frame::silence();

    for ev in events {
        let n = ((ev.dur_ms * SR / 1000.0) as usize).max(1);

        for i in 0..n {
            let pos = i as f32 / n as f32;
            let f0  = ev.pitch_start + (ev.pitch_end - ev.pitch_start) * pos;

            // Target frame for this position within the phoneme
            let target = frame_at(&ev.params, pos);

            // Smooth transition from previous phoneme at the start
            let fr = if i < trans_n {
                let t = i as f32 / trans_n as f32;
                prev.lerp(&target, t)
            } else {
                target
            };

            // Update resonators
            bp[0].set_freq(fr.f1, fr.bw1, SR);
            bp[1].set_freq(fr.f2, fr.bw2, SR);
            bp[2].set_freq(fr.f3, fr.bw3, SR);

            // Excitation: glottal pulse + noise blend
            let pulse  = glottal_pulse(phase, EFFORT);
            let noise  = rng.next_f32();
            let vg     = 1.0 - fr.aspiration * 0.85;
            let exc    = fr.voicing       * pulse * vg
                       + (1.0 - fr.voicing) * noise * 0.35
                       + fr.aspiration    * noise * 0.5;

            phase += f0 / SR;
            if phase >= 1.0 { phase -= 1.0; }

            // Filter bank
            let y = fr.a1 * bp[0].process(exc)
                  + fr.a2 * bp[1].process(exc)
                  + fr.a3 * bp[2].process(exc);

            samples.push(soft_clip(y * GAIN));
        }

        // Save the end-frame of this phoneme for the next transition
        prev = frame_at(&ev.params, 1.0);
    }

    // Peak-normalise to ~91 % of i16 range
    let peak = samples.iter().copied().fold(0.0f32, |m, s| m.max(s.abs()));
    let scale = if peak > 1e-6 { 30000.0 / peak } else { 1.0 };
    samples.iter().map(|&s| (s * scale).clamp(-32768.0, 32767.0) as i16).collect()
}
