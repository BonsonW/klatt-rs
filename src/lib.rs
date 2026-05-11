mod dsp;
pub mod phonemes;
mod synth;

pub use synth::{synthesize, SynthEvent, SAMPLE_RATE};
