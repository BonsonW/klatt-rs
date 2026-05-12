use std::path::{Path, PathBuf};

use espeak_ng::{PhonemeData, Translator};

use crate::phonemes;
use crate::synth::{SynthEvent, VoiceSettings};

const PH_VOWEL: u8 = 2;

/// Options for [`text_to_events`].
pub struct TtsOptions {
    /// Language code passed to espeak-ng (e.g. `"en"`, `"de"`).
    pub lang: &'static str,
    /// Automatically apply rising pitch contour when the text ends with `?`.
    pub question_intonation: bool,
}

impl Default for TtsOptions {
    fn default() -> Self {
        Self { lang: "en", question_intonation: true }
    }
}

/// Install bundled espeak-ng language data and return the data directory path.
///
/// Requires the `bundled-data-en` (or equivalent) Cargo feature to be enabled.
/// Safe to call multiple times — existing files are overwritten in place.
///
/// `path` — where to install the data. Pass `None` to use a default directory
/// inside the OS temp folder (`<tmp>/klatt-rs-espeak-data`).
#[cfg(feature = "bundled-data-en")]
pub fn install_language(lang: &str, path: Option<&std::path::Path>) -> PathBuf {
    let data_dir: PathBuf = match path {
        Some(p) => p.to_path_buf(),
        None    => std::env::temp_dir().join("klatt-rs-espeak-data"),
    };
    std::fs::create_dir_all(&data_dir).expect("create espeak data dir");
    espeak_ng::install_bundled_language(&data_dir, lang).expect("install espeak language data");
    data_dir
}

/// Convert text to a sequence of [`SynthEvent`]s using espeak-ng for phonemisation.
///
/// `data_dir` must point to a directory prepared by [`install_language`] or
/// `espeak_ng::install_bundled_language`.
pub fn text_to_events(text: &str, data_dir: &Path, v: &VoiceSettings, opts: &TtsOptions) -> Vec<SynthEvent> {
    let mut phdata = PhonemeData::load(data_dir).expect("load phoneme data");
    phdata.select_table_by_name(opts.lang).expect("select language");
    let translator = Translator::new(opts.lang, Some(data_dir)).expect("create translator");
    let codes = translator.translate_to_codes(text).expect("translate");

    let mut events: Vec<SynthEvent> = Vec::new();
    let mut pending_stress: u8 = 1;

    for code in &codes {
        if code.is_boundary { continue; }
        match code.code {
            0 => {}
            1..=7 => { pending_stress = code.code; }
            n => {
                if let Some(ph) = phdata.get(n) {
                    let mnem     = unpack_mnem(ph.mnemonic);
                    let is_vowel = ph.typ == PH_VOWEL;

                    if let Some(params) = phonemes::lookup(&mnem) {
                        let dur_ms = compute_duration(
                            ph.std_length, pending_stress, is_vowel, params.dur_ms,
                        ) * v.rate;

                        let (pitch_start, pitch_end) = if is_vowel {
                            let pct = match pending_stress {
                                6 | 7 => 1.15f32,
                                4 | 5 => 1.07,
                                _     => 0.92,
                            };
                            let ps = v.base_pitch_hz * pct;
                            (ps, ps * 0.93)
                        } else {
                            (v.base_pitch_hz, v.base_pitch_hz)
                        };

                        events.push(SynthEvent { params, dur_ms, pitch_start, pitch_end });
                    }

                    if is_vowel { pending_stress = 1; }
                }
            }
        }
    }

    events.push(SynthEvent {
        params:      phonemes::lookup("_").unwrap(),
        dur_ms:      200.0 * v.rate,
        pitch_start: v.base_pitch_hz,
        pitch_end:   v.base_pitch_hz,
    });

    if opts.question_intonation && text.trim_end().ends_with('?') {
        apply_question_intonation(&mut events);
    }

    events
}

/// Apply a rising pitch contour to the last few voiced events, suitable for yes/no questions.
///
/// Called automatically by [`text_to_events`] when [`TtsOptions::question_intonation`] is set
/// and the text ends with `?`. Exposed publicly for manual use in other intonation pipelines.
pub fn apply_question_intonation(events: &mut Vec<SynthEvent>) {
    const RISE_COUNT: usize = 5;
    const PEAK_SCALE: f32 = 1.4;
    let voiced_indices: Vec<usize> = events.iter().enumerate()
        .filter(|(_, ev)| ev.params.voicing > 0.3)
        .map(|(i, _)| i)
        .collect();
    let n = voiced_indices.len().min(RISE_COUNT);
    if n == 0 { return; }
    let slice = &voiced_indices[voiced_indices.len() - n..];
    for (step, &idx) in slice.iter().enumerate() {
        let t = (step + 1) as f32 / n as f32;
        let scale = 1.0 + (PEAK_SCALE - 1.0) * t;
        events[idx].pitch_start *= scale;
        events[idx].pitch_end   *= scale * 1.05;
    }
}

fn unpack_mnem(v: u32) -> String {
    let mut s = String::with_capacity(4);
    let mut v = v;
    for _ in 0..4 {
        let c = (v & 0xff) as u8;
        if c != 0 { s.push(c as char); }
        v >>= 8;
    }
    s
}

fn compute_duration(std_length: u8, stress_code: u8, is_vowel: bool, consonant_default_ms: f32) -> f32 {
    if !is_vowel {
        return consonant_default_ms.max(65.0);
    }
    let stress_len: f32 = match stress_code {
        0 => 170.0, 1 => 135.0, 2 => 205.0, 3 => 205.0,
        4 => 180.0, 5 => 200.0, 6 => 245.0, 7 => 275.0, _ => 135.0,
    };
    ((std_length as f32) * stress_len / 370.0).max(70.0)
}
