use std::f32::consts::TAU;

use crate::dsp::{glottal_pulse, soft_clip, BandpassBiquad, Xorshift32};
use crate::phonemes::FormantParams;

pub const SAMPLE_RATE: u32 = 22050;
const SR: f32 = SAMPLE_RATE as f32;
const GAIN:          f32 = 3.5;
const TRANSITION_MS: f32 = 20.0;

// ── VoiceSettings ─────────────────────────────────────────────────────────────

/// Voice parameters passed to [`synthesize`].  All fields have sensible
/// defaults via [`VoiceSettings::default`].
pub struct VoiceSettings {
    /// Fundamental frequency in Hz. (120 = typical male, 180 = typical female)
    pub base_pitch_hz: f32,
    /// Duration multiplier applied to every phoneme.  1.0 = normal speed,
    /// 0.5 = twice as fast, 2.0 = twice as slow.
    pub rate: f32,
    /// Multiplicative scale applied to all formant frequencies (F1/F2/F3).
    /// Values > 1.0 shift resonances upward (brighter/smaller tract),
    /// values < 1.0 shift them downward (darker/larger tract).
    pub formant_scale: f32,
    /// Vibrato depth in Hz (peak F0 deviation).  0 = off.
    pub vibrato_depth: f32,
    /// Vibrato LFO rate in Hz.
    pub vibrato_rate: f32,
    /// Tremolo depth 0..1 (amplitude modulation).  0 = off.
    pub tremolo_depth: f32,
    /// Tremolo LFO rate in Hz.
    pub tremolo_rate: f32,
    /// Breathiness added to every phoneme on top of its own aspiration.
    /// Clamped so total aspiration stays in 0..1.
    pub aspiration: f32,
    /// Spectral tilt.  0 = flat, positive values roll off high frequencies
    /// (darker), negative values boost them (brighter).  Range −0.95..0.95.
    pub tilt: f32,
    /// Glottal pulse shape 0..1.  0 = lax/breathy, 1 = tense/creaky.
    pub effort: f32,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            base_pitch_hz: 120.0,
            rate:          1.0,
            formant_scale: 1.0,
            vibrato_depth: 0.0,
            vibrato_rate:  5.0,
            tremolo_depth: 0.0,
            tremolo_rate:  5.0,
            aspiration:    0.0,
            tilt:          0.0,
            effort:        0.5,
        }
    }
}

// ── SynthEvent ────────────────────────────────────────────────────────────────

pub struct SynthEvent {
    pub params:      FormantParams,
    pub dur_ms:      f32,
    pub pitch_start: f32,
    pub pitch_end:   f32,
}

// ── Internal frame type ───────────────────────────────────────────────────────

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
            f1: 500.0, bw1:  80.0, a1: 0.0,
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

fn frame_at(p: &FormantParams, pos: f32) -> Frame {
    if p.is_stop {
        return if pos < 0.40 {
            Frame { voicing: 0.0, a1: 0.0, a2: 0.0, a3: 0.0, aspiration: 0.0,
                f1: p.f1, bw1: p.bw1, f2: p.f2, bw2: p.bw2, f3: p.f3, bw3: p.bw3 }
        } else if pos < 0.60 {
            // Voiced stops (b, d, g) get a soft burst; unvoiced (p, t, k) get full noise plosion.
            let (burst_voicing, burst_asp) = if p.voicing > 0.3 { (p.voicing * 0.4, 0.25) } else { (0.0, 0.8) };
            Frame { voicing: burst_voicing, aspiration: burst_asp,
                f1: p.f1, bw1: p.bw1, a1: p.a1,
                f2: p.f2, bw2: p.bw2, a2: p.a2,
                f3: p.f3, bw3: p.bw3, a3: p.a3 }
        } else {
            // Release: decay voiced stops so the formant ring-down doesn't sound like a vowel.
            let mut fr = Frame::from_params(p);
            if p.voicing > 0.3 {
                let decay = 1.0 - (pos - 0.60) / 0.40;  // 1.0 at burst end → 0.0 at phoneme boundary
                fr.voicing *= decay;
                fr.a1      *= decay;
                fr.a2      *= decay;
                fr.a3      *= decay;
            }
            fr
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

// ── Synthesis ─────────────────────────────────────────────────────────────────

pub fn synthesize(events: &[SynthEvent], settings: &VoiceSettings) -> Vec<i16> {
    let trans_n = (TRANSITION_MS * SR / 1000.0) as usize;

    let mut out:   Vec<f32> = Vec::new();
    let mut bp     = [BandpassBiquad::new(), BandpassBiquad::new(), BandpassBiquad::new()];
    let mut rng    = Xorshift32::new(0x12345678);
    let mut phase  = 0.0f32;
    let mut tilt_y = 0.0f32;   // previous output sample for tilt filter
    let mut t_samp = 0usize;   // global sample counter for LFOs

    let mut prev = Frame::silence();

    for ev in events {
        let n = ((ev.dur_ms * SR / 1000.0) as usize).max(1);

        for i in 0..n {
            let t      = t_samp as f32 / SR;
            let pos    = i as f32 / n as f32;

            // ── F0 with vibrato ──────────────────────────────────────────────
            let f0_base = ev.pitch_start + (ev.pitch_end - ev.pitch_start) * pos;
            let f0 = f0_base
                + settings.vibrato_depth * (TAU * settings.vibrato_rate * t).sin();

            // ── Frame with coarticulation crossfade ──────────────────────────
            let target = frame_at(&ev.params, pos);
            let fr = if i < trans_n {
                prev.lerp(&target, i as f32 / trans_n as f32)
            } else {
                target
            };

            // ── Formant filters ──────────────────────────────────────────────
            let sc = settings.formant_scale;
            bp[0].set_freq(fr.f1 * sc, fr.bw1, SR);
            bp[1].set_freq(fr.f2 * sc, fr.bw2, SR);
            bp[2].set_freq(fr.f3 * sc, fr.bw3, SR);

            // ── Excitation ───────────────────────────────────────────────────
            let asp   = (fr.aspiration + settings.aspiration).min(1.0);
            let pulse = glottal_pulse(phase, settings.effort);
            let noise = rng.next_f32();
            let vg    = 1.0 - asp * 0.85;
            let exc   = fr.voicing         * pulse * vg
                      + (1.0 - fr.voicing) * noise * 0.35
                      + asp                * noise * 0.5;

            phase += f0 / SR;
            if phase >= 1.0 { phase -= 1.0; }

            // ── Filter bank ──────────────────────────────────────────────────
            let y = fr.a1 * bp[0].process(exc)
                  + fr.a2 * bp[1].process(exc)
                  + fr.a3 * bp[2].process(exc);

            // ── Spectral tilt ────────────────────────────────────────────────
            let y = y - settings.tilt * tilt_y;
            tilt_y = y;

            // ── Tremolo + gain ───────────────────────────────────────────────
            let tremolo = 1.0
                - settings.tremolo_depth
                  * (0.5 + 0.5 * (TAU * settings.tremolo_rate * t).sin());
            out.push(soft_clip(y * GAIN * tremolo));

            t_samp += 1;
        }

        prev = frame_at(&ev.params, 1.0);
    }

    // Peak-normalise to ~91 % of i16 range
    let peak = out.iter().copied().fold(0.0f32, |m, s| m.max(s.abs()));
    let scale = if peak > 1e-6 { 30000.0 / peak } else { 1.0 };
    out.iter().map(|&s| (s * scale).clamp(-32768.0, 32767.0) as i16).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phonemes;

    fn voice() -> VoiceSettings { VoiceSettings { base_pitch_hz: 200.0, ..Default::default() } }

    fn event(mnem: &str, dur_ms: f32) -> SynthEvent {
        SynthEvent {
            params:      phonemes::lookup(mnem).unwrap(),
            dur_ms,
            pitch_start: 200.0,
            pitch_end:   200.0,
        }
    }

    // ── Output length ────────────────────────────────────────────────────────────

    #[test]
    fn output_length_matches_duration() {
        let ev = event("@", 200.0);
        let n_expected = ((200.0 * SR / 1000.0) as usize).max(1);
        assert_eq!(synthesize(&[ev], &voice()).len(), n_expected);
    }

    #[test]
    fn multiple_events_concatenate() {
        let events = ["@", "m", "@"].map(|m| event(m, 100.0));
        let n_expected = 3 * ((100.0 * SR / 1000.0) as usize).max(1);
        assert_eq!(synthesize(&events, &voice()).len(), n_expected);
    }

    // ── Silence and signal ───────────────────────────────────────────────────────

    #[test]
    fn voiced_phoneme_produces_nonzero_output() {
        let samples = synthesize(&[event("i", 100.0)], &voice());
        assert!(samples.iter().any(|&s| s != 0), "voiced phoneme produced silence");
    }

    #[test]
    fn silence_phoneme_is_near_silent() {
        let samples = synthesize(&[event("_", 100.0)], &voice());
        let peak = samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
        assert!(peak < 100, "silence phoneme too loud: peak={peak}");
    }

    #[test]
    fn stop_hold_phase_is_silent() {
        // First 40 % of a stop is the held closure — should be near-silent.
        let dur_ms = 120.0;
        let samples = synthesize(&[event("k", dur_ms)], &voice());
        let hold_end = (samples.len() as f32 * 0.38) as usize;
        let peak = samples[..hold_end].iter().map(|&s| s.abs()).max().unwrap_or(0);
        assert!(peak < 50, "stop hold phase too loud: peak={peak}");
    }

    // ── Normalisation ────────────────────────────────────────────────────────────

    #[test]
    fn output_does_not_clip() {
        let events = ["i", "A", "u"].map(|m| event(m, 100.0));
        let samples = synthesize(&events, &voice());
        assert!(samples.iter().all(|&s| s > i16::MIN && s < i16::MAX));
    }

    #[test]
    fn voiced_output_is_normalised_near_peak() {
        let samples = synthesize(&[event("A", 200.0)], &voice());
        let peak = samples.iter().map(|&s| s.abs()).max().unwrap_or(0);
        assert!(peak > 20_000, "output not normalised — peak={peak}");
    }
}
