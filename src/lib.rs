mod dsp;
pub mod phonemes;
mod synth;

pub use synth::{synthesize, SynthEvent, VoiceSettings, SAMPLE_RATE};
