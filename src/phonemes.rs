/// Klatt (1980) formant parameters, keyed by espeak X-SAMPA mnemonic.
#[derive(Clone)]
pub struct FormantParams {
    pub voicing:    f32,
    pub f1:  f32, pub bw1: f32, pub a1: f32,
    pub f2:  f32, pub bw2: f32, pub a2: f32,
    pub f3:  f32, pub bw3: f32, pub a3: f32,
    pub aspiration: f32,
    pub is_stop:    bool,
    /// [F1, F2, F3] glide targets for diphthongs.
    pub glide_to:   Option<[f32; 3]>,
    /// Default duration in ms (used when espeak reports std_length=0 for consonants).
    pub dur_ms:     f32,
}

impl FormantParams {
    const fn mono(
        voicing: f32,
        f1: f32, bw1: f32, a1: f32,
        f2: f32, bw2: f32, a2: f32,
        f3: f32, bw3: f32, a3: f32,
        aspiration: f32,
        is_stop: bool,
        dur_ms: f32,
    ) -> Self {
        Self { voicing, f1, bw1, a1, f2, bw2, a2, f3, bw3, a3,
               aspiration, is_stop, glide_to: None, dur_ms }
    }

    fn diph(
        f1: f32, f2: f32, f3: f32,
        bw1: f32, bw2: f32, bw3: f32,
        gf1: f32, gf2: f32, gf3: f32,
        dur_ms: f32,
    ) -> Self {
        Self {
            voicing: 1.0,
            f1, bw1, a1: 1.0,
            f2, bw2, a2: 0.9,
            f3, bw3, a3: 0.7,
            aspiration: 0.0,
            is_stop: false,
            glide_to: Some([gf1, gf2, gf3]),
            dur_ms,
        }
    }
}

// Convenience shorthands used in the table below.
const V : f32 = 1.0;   // fully voiced
const UV: f32 = 0.0;   // unvoiced
const PV: f32 = 0.45;  // partially voiced fricative
const SV: f32 = 0.6;   // stop voiced release
const AV: f32 = 0.5;   // affricate voiced

/// Look up espeak X-SAMPA mnemonic → Klatt formant params.
/// Strips a trailing digit or `#` before the second lookup attempt.
pub fn lookup(mnem: &str) -> Option<FormantParams> {
    if let Some(p) = lookup_exact(mnem) { return Some(p); }
    // Strip variant suffix (trailing digit or '#')
    let bytes = mnem.as_bytes();
    if bytes.len() >= 2 {
        let last = *bytes.last().unwrap();
        if last.is_ascii_digit() || last == b'#' {
            let base = &mnem[..mnem.len()-1];
            if let Some(p) = lookup_exact(base) { return Some(p); }
        }
    }
    None
}

fn lookup_exact(m: &str) -> Option<FormantParams> {
    // Formant data from Klatt (1980) via klattsch (MIT licensed).
    // Bandwidths and amplitudes follow the published tables.
    // dur_ms for vowels is 0.0 (computed from espeak std_length + stress);
    // for consonants it is the class-based default used when std_length == 0.
    let p = match m {
        // ── Vowels ──────────────────────────────────────────────────────────
        //          voicing  F1    BW1   A1    F2    BW2   A2    F3    BW3   A3   asp    stop   dur
        "i" | "i:" => FormantParams::mono(V,  310.0, 45.0, 1.0, 2020.0,200.0, 0.9, 2960.0, 400.0, 0.7, 0.0, false, 0.0),
        "I" | "I2" => FormantParams::mono(V,  400.0, 50.0, 1.0, 1800.0,100.0, 0.9, 2570.0, 140.0, 0.7, 0.0, false, 0.0),
        "E" | "e"       => FormantParams::mono(V,  530.0, 60.0, 1.0, 1680.0, 90.0, 0.9, 2500.0, 200.0, 0.7, 0.0, false, 0.0),
        "{"        => FormantParams::mono(V,  620.0, 70.0, 1.0, 1660.0,150.0, 0.9, 2430.0, 320.0, 0.7, 0.0, false, 0.0),
        "A" | "A:" | "a" | "a:" | "A@" => FormantParams::mono(V, 700.0,130.0, 1.0, 1220.0, 70.0, 0.9, 2600.0, 160.0, 0.7, 0.0, false, 0.0),
        "Q" | "O" | "O:" | "0" => FormantParams::mono(V,   600.0, 90.0, 1.0,  990.0,100.0, 0.9, 2570.0,  80.0, 0.7, 0.0, false, 0.0),
        "V"        => FormantParams::mono(V,  620.0, 80.0, 1.0, 1220.0, 50.0, 0.9, 2550.0, 140.0, 0.7, 0.0, false, 0.0),
        "@" | "@2" | "@L" => FormantParams::mono(V, 500.0, 80.0, 1.0, 1500.0,120.0, 0.9, 2500.0, 160.0, 0.7, 0.0, false, 0.0),
        "U" | "U2" => FormantParams::mono(V,  450.0, 80.0, 1.0, 1100.0,100.0, 0.9, 2350.0,  80.0, 0.7, 0.0, false, 0.0),
        "u" | "u:" => FormantParams::mono(V,  350.0, 65.0, 1.0, 1250.0,110.0, 0.9, 2200.0, 140.0, 0.7, 0.0, false, 0.0),
        "3" | "3:" | "@@" => FormantParams::mono(V, 470.0,100.0, 1.0, 1270.0, 60.0, 0.9, 1540.0, 110.0, 0.7, 0.0, false, 0.0),
        // ── Diphthongs ──────────────────────────────────────────────────────
        //                    start F1    F2     F3    BW1  BW2  BW3   end F1   F2     F3    dur
        "aI" | "aI;" | "aI2" => FormantParams::diph(660.0,1200.0,2550.0, 100.0,70.0,200.0, 400.0,1880.0,2500.0, 0.0),
        "eI"                 => FormantParams::diph(480.0,1720.0,2520.0,  70.0,100.0,200.0, 330.0,2020.0,2600.0, 0.0),
        "OI" | "Oi"          => FormantParams::diph(550.0, 960.0,2400.0,  80.0,50.0,130.0, 360.0,1820.0,2450.0, 0.0),
        "aU" | "aU;"         => FormantParams::diph(640.0,1230.0,2550.0,  80.0,70.0,140.0, 420.0, 940.0,2350.0, 0.0),
        "oU" | "@U" | "oU;"  => FormantParams::diph(540.0,1100.0,2300.0,  80.0,70.0, 70.0, 450.0, 900.0,2300.0, 0.0),
        // ER-centring diphthongs
        "I@" | "i@" => FormantParams::diph(400.0,1800.0,2570.0, 50.0,100.0,140.0, 470.0,1270.0,1540.0, 0.0),
        "e@" => FormantParams::diph(530.0,1680.0,2500.0, 60.0, 90.0,200.0, 470.0,1270.0,1540.0, 0.0),
        "U@" => FormantParams::diph(450.0,1100.0,2350.0, 80.0,100.0, 80.0, 470.0,1270.0,1540.0, 0.0),
        // ── Approximants ────────────────────────────────────────────────────
        "w"         => FormantParams::mono(V,  290.0, 50.0, 0.8,  610.0, 80.0, 0.7, 2150.0,  60.0, 0.5, 0.0, false,  75.0),
        "j"         => FormantParams::mono(V,  260.0, 40.0, 0.8, 2070.0,250.0, 0.7, 3020.0, 500.0, 0.5, 0.0, false,  70.0),
        "r" | "r\\" => FormantParams::mono(V,  310.0, 70.0, 0.8, 1060.0,100.0, 0.7, 1380.0, 120.0, 0.5, 0.0, false,  75.0),
        "l"         => FormantParams::mono(V,  310.0, 50.0, 0.8, 1050.0,100.0, 0.7, 2880.0, 280.0, 0.5, 0.0, false,  75.0),
        // ── Nasals ──────────────────────────────────────────────────────────
        "m"         => FormantParams::mono(V,  270.0, 40.0, 0.7, 1270.0,200.0, 0.18, 2130.0, 200.0, 0.10, 0.0, false, 85.0),
        "n"         => FormantParams::mono(V,  270.0, 40.0, 0.7, 1340.0,300.0, 0.20, 2470.0, 300.0, 0.12, 0.0, false, 85.0),
        "N"         => FormantParams::mono(V,  270.0, 40.0, 0.7, 2000.0,300.0, 0.20, 2700.0, 300.0, 0.12, 0.0, false, 90.0),
        // ── Unvoiced fricatives ──────────────────────────────────────────────
        "f"         => FormantParams::mono(UV, 340.0,200.0, 0.0, 1100.0,200.0, 0.10, 2080.0,1000.0, 0.15, 0.2, false, 110.0),
        "T"         => FormantParams::mono(UV, 400.0,300.0, 0.0, 1600.0,120.0, 0.30, 2600.0, 250.0, 0.55, 0.0, true,   80.0),
        "s"         => FormantParams::mono(UV, 320.0,200.0, 0.0, 1390.0,200.0, 0.00, 5500.0,1000.0, 0.95, 0.2, false, 120.0),
        "S"         => FormantParams::mono(UV, 300.0,200.0, 0.0, 1840.0,200.0, 0.55, 2750.0,1000.0, 0.65, 0.2, false, 110.0),
        "x"         => FormantParams::mono(UV, 300.0,200.0, 0.0, 1500.0,300.0, 0.30, 2500.0, 800.0, 0.40, 0.4, false, 100.0),
        // ── Voiced fricatives ────────────────────────────────────────────────
        "v"         => FormantParams::mono(PV, 220.0, 80.0, 0.4, 1100.0,100.0, 0.12, 2080.0, 800.0, 0.18, 0.1, false, 100.0),
        "D"         => FormantParams::mono(SV, 200.0, 60.0, 0.5, 1600.0,100.0, 0.40, 2600.0, 170.0, 0.50, 0.0, true,   80.0),
        "z"         => FormantParams::mono(PV, 240.0, 80.0, 0.4, 1390.0,100.0, 0.00, 5500.0, 800.0, 0.65, 0.1, false, 110.0),
        "Z"         => FormantParams::mono(PV, 270.0, 80.0, 0.4, 1840.0,100.0, 0.45, 2750.0, 800.0, 0.55, 0.1, false, 100.0),
        // ── Aspiration ───────────────────────────────────────────────────────
        "h"         => FormantParams::mono(UV, 500.0,300.0, 0.4, 1500.0,200.0, 0.40, 2500.0, 300.0, 0.30, 0.7, false,  90.0),
        // ── Stops – unvoiced ─────────────────────────────────────────────────
        "p"         => FormantParams::mono(UV, 400.0,300.0, 0.1, 1100.0,150.0, 0.20, 2150.0, 220.0, 0.25, 0.0, true,   80.0),
        "t"         => FormantParams::mono(UV, 400.0,300.0, 0.0, 1600.0,120.0, 0.30, 2600.0, 250.0, 0.55, 0.0, true,   80.0),
        "k"         => FormantParams::mono(UV, 300.0,250.0, 0.0, 1990.0,160.0, 0.50, 2850.0, 330.0, 0.40, 0.0, true,   85.0),
        // ── Stops – voiced ───────────────────────────────────────────────────
        "b"         => FormantParams::mono(SV, 200.0, 60.0, 0.5, 1100.0,110.0, 0.20, 2150.0, 130.0, 0.20, 0.0, true,   80.0),
        "d"         => FormantParams::mono(SV, 200.0, 60.0, 0.5, 1600.0,100.0, 0.40, 2600.0, 170.0, 0.50, 0.0, true,   80.0),
        "g"         => FormantParams::mono(SV, 200.0, 60.0, 0.5, 1990.0,150.0, 0.50, 2850.0, 280.0, 0.40, 0.0, true,   85.0),
        // ── Affricates ───────────────────────────────────────────────────────
        "tS"        => FormantParams::mono(UV, 350.0,200.0, 0.0, 1800.0, 90.0, 0.40, 2820.0, 300.0, 0.55, 0.0, true,  110.0),
        "dZ"        => FormantParams::mono(AV, 260.0, 60.0, 0.4, 1800.0, 80.0, 0.40, 2820.0, 270.0, 0.50, 0.0, true,  110.0),
        // ── Glottal stop / silence ───────────────────────────────────────────
        "?" | "_"   => FormantParams::mono(UV, 500.0, 80.0, 0.0, 1500.0,120.0, 0.00, 2500.0, 160.0, 0.00, 0.0, false,   0.0),
        _ => return None,
    };
    Some(p)
}
