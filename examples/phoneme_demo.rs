//! Synthesises every common phoneme in isolation (vowels) or in a schwa context
//! (ə-X-ə for consonants) and writes a single annotated WAV to /tmp/phoneme_demo.wav.
//!
//! Run with:
//!   cargo run --example phoneme_demo --manifest-path klatt-rs/Cargo.toml

use std::io::Write;

use klatt_rs::{phonemes, synthesize, SynthEvent, VoiceSettings, SAMPLE_RATE};

// (X-SAMPA mnemonic, display label, is_vowel)
const PHONEMES: &[(&str, &str, bool)] = &[
    // ── Vowels ───────────────────────────────────────────────────────────────
    ("i:",  "iː  fleece",    true),
    ("I",   "ɪ   kit",       true),
    ("e",   "e   dress",     true),
    ("E",   "ɛ   bed",       true),
    ("{",   "æ   trap",      true),
    ("A:",  "ɑː  palm",      true),
    ("Q",   "ɒ   lot",       true),
    ("O:",  "ɔː  thought",   true),
    ("U",   "ʊ   foot",      true),
    ("u:",  "uː  goose",     true),
    ("V",   "ʌ   strut",     true),
    ("@",   "ə   comma",     true),
    ("3:",  "ɜː  nurse",     true),
    // ── Diphthongs ───────────────────────────────────────────────────────────
    ("eI",  "eɪ  face",      true),
    ("aI",  "aɪ  price",     true),
    ("OI",  "ɔɪ  choice",    true),
    ("aU",  "aʊ  mouth",     true),
    ("@U",  "əʊ  goat",      true),
    ("I@",  "ɪə  near",      true),
    ("e@",  "eə  square",    true),
    ("U@",  "ʊə  cure",      true),
    // ── Approximants ─────────────────────────────────────────────────────────
    ("w",   "w   wet",       false),
    ("j",   "j   yes",       false),
    ("r",   "r   red",       false),
    ("l",   "l   led",       false),
    ("5",   "ɫ   milk (dark-l)", false),
    // ── Nasals ───────────────────────────────────────────────────────────────
    ("m",   "m   map",       false),
    ("n",   "n   nap",       false),
    ("N",   "ŋ   sing",      false),
    // ── Stops ────────────────────────────────────────────────────────────────
    ("p",   "p   pen",       false),
    ("b",   "b   bed",       false),
    ("t",   "t   ten",       false),
    ("d",   "d   den",       false),
    ("k",   "k   cat",       false),
    ("g",   "g   get",       false),
    // ── Fricatives ───────────────────────────────────────────────────────────
    ("f",   "f   fan",       false),
    ("v",   "v   van",       false),
    ("T",   "θ   thin",      false),
    ("D",   "ð   then",      false),
    ("s",   "s   sip",       false),
    ("z",   "z   zip",       false),
    ("S",   "ʃ   ship",      false),
    ("Z",   "ʒ   measure",   false),
    ("h",   "h   hat",       false),
    // ── Affricates ───────────────────────────────────────────────────────────
    ("tS",  "tʃ  chin",      false),
    ("dZ",  "dʒ  gin",       false),
];

fn main() {
    let voice = VoiceSettings {
        base_pitch_hz: 200.0,
        rate:          1.0,
        formant_scale: 1.0,
        ..VoiceSettings::default()
    };

    let schwa  = phonemes::lookup("@").expect("schwa");
    let silence = phonemes::lookup("_").expect("silence");

    let mut all_samples: Vec<i16> = Vec::new();
    let mut missing: Vec<&str>    = Vec::new();

    for &(mnem, label, is_vowel) in PHONEMES {
        let Some(params) = phonemes::lookup(mnem) else {
            eprintln!("  [MISSING] {mnem}");
            missing.push(mnem);
            continue;
        };

        let mut events: Vec<SynthEvent> = Vec::new();

        if !is_vowel {
            events.push(SynthEvent {
                params:      schwa.clone(),
                dur_ms:      130.0,
                pitch_start: voice.base_pitch_hz,
                pitch_end:   voice.base_pitch_hz,
            });
        }

        let dur_ms = if is_vowel { 280.0 } else { 110.0 };
        events.push(SynthEvent {
            params,
            dur_ms,
            pitch_start: voice.base_pitch_hz,
            pitch_end:   voice.base_pitch_hz * 0.93,
        });

        if !is_vowel {
            events.push(SynthEvent {
                params:      schwa.clone(),
                dur_ms:      130.0,
                pitch_start: voice.base_pitch_hz * 0.93,
                pitch_end:   voice.base_pitch_hz * 0.88,
            });
        }

        // Inter-phoneme pause
        events.push(SynthEvent {
            params:      silence.clone(),
            dur_ms:      140.0,
            pitch_start: voice.base_pitch_hz,
            pitch_end:   voice.base_pitch_hz,
        });

        let chunk = synthesize(&events, &voice);
        let t_ms  = all_samples.len() as f32 / SAMPLE_RATE as f32 * 1000.0;
        println!("{:6.0} ms  {mnem:5}  {label}", t_ms);
        all_samples.extend_from_slice(&chunk);
    }

    let path = "/tmp/phoneme_demo.wav";
    write_wav(path, &all_samples, SAMPLE_RATE).expect("write WAV");
    println!("\nwrote {path}  ({} samples, {:.1} s)",
        all_samples.len(),
        all_samples.len() as f32 / SAMPLE_RATE as f32);

    if !missing.is_empty() {
        eprintln!("\nMissing phonemes: {missing:?}");
    }
}

fn write_wav(path: &str, samples: &[i16], sample_rate: u32) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    let data_len = (samples.len() * 2) as u32;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVE")?;
    f.write_all(b"fmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?;
    f.write_all(&sample_rate.to_le_bytes())?;
    f.write_all(&(sample_rate * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for s in samples { f.write_all(&s.to_le_bytes())?; }
    Ok(())
}
