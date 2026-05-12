mod dsp;
pub mod frontend;
pub mod phonemes;
mod synth;

pub use frontend::TtsOptions;
pub use synth::{synthesize, SynthEvent, VoiceSettings, SAMPLE_RATE};

pub mod prelude {
    pub use crate::frontend::{apply_question_intonation, text_to_events};
    pub use crate::{synthesize, SynthEvent, TtsOptions, VoiceSettings, SAMPLE_RATE};

    #[cfg(feature = "bundled-data-en")]
    pub use crate::frontend::install_language;
}
