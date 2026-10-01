//! Fixed-memory, single-precision monophonic frequency detector.

const WINDOW: usize = 128;
const HISTORY: usize = 256;
const HOP: usize = 64;
const TARGET_RATE: f32 = 8_000.0;
const MIN_HZ: f32 = 70.0;
const MAX_HZ: f32 = 1_300.0;
const SILENCE_POWER: f32 = 0.000_025; // -46 dBFS RMS
const YIN_THRESHOLD: f32 = 0.15;

pub struct FrequencyDetector {
    rate: f32,
    decimation: usize,
    decimation_phase: usize,
    lowpass: f32,
    lowpass_alpha: f32,
    history: [f32; HISTORY],
    cursor: usize,
    filled: usize,
    since_analysis: usize,
    min_lag: usize,
    max_lag: usize,
    frequency: f32,
}

impl FrequencyDetector {
    pub fn new(sample_rate: f32) -> Option<Self> {
        if !sample_rate.is_finite() || !(8_000.0..=384_000.0).contains(&sample_rate) {
            return None;
        }
        let decimation = ((sample_rate / TARGET_RATE + 0.5) as usize).max(1);
        let rate = sample_rate / decimation as f32;
        let min_lag = ((rate / MAX_HZ) as usize).max(2);
        let max_lag = (rate / MIN_HZ) as usize + 1;
        if max_lag >= HISTORY - WINDOW {
            return None;
        }
        Some(Self {
            rate,
            decimation,
            decimation_phase: 0,
            lowpass: 0.0,
            lowpass_alpha: 1.0 / decimation as f32,
            history: [0.0; HISTORY],
            cursor: 0,
            filled: 0,
            since_analysis: 0,
            min_lag,
            max_lag,
            frequency: 0.0,
        })
    }

    pub fn reset(&mut self) {
        self.decimation_phase = 0;
        self.lowpass = 0.0;
        self.history.fill(0.0);
        self.cursor = 0;
        self.filled = 0;
        self.since_analysis = 0;
        self.frequency = 0.0;
    }

    pub fn frequency_hz(&self) -> f32 {
        self.frequency
    }

    pub fn push(&mut self, sample: f32) {
        // Treat corrupt input as silence; it must not poison filter state.
        let sample = if sample.is_finite() { sample } else { 0.0 };
        self.lowpass += self.lowpass_alpha * (sample - self.lowpass);
        self.decimation_phase += 1;
        if self.decimation_phase < self.decimation {
            return;
        }
        self.decimation_phase = 0;

        self.history[self.cursor & (HISTORY - 1)] = self.lowpass;
        self.cursor = (self.cursor + 1) & (HISTORY - 1);
        self.filled = (self.filled + 1).min(HISTORY);
        self.since_analysis += 1;
        if self.filled == HISTORY && self.since_analysis >= HOP {
            self.since_analysis = 0;
            self.analyse();
        }
    }

    fn analyse(&mut self) {
        // A fixed comparison length avoids favouring longer lags.
        let mut power = 0.0;
        for i in 0..WINDOW {
            let x = self.history[(self.cursor + HISTORY - WINDOW + i) & (HISTORY - 1)];
            power += x * x;
        }
        if power < SILENCE_POWER * WINDOW as f32 {
            self.frequency = 0.0;
            return;
        }

        // YIN's cumulative mean-normalised difference suppresses harmonics.
        // Keep only three values for parabolic interpolation, with no heap use.
        let mut cumulative = 0.0;
        let mut previous = 1.0;
        let mut candidate: Option<(usize, f32, f32)> = None;
        let mut best = (0usize, 1.0f32, 1.0f32, 1.0f32);
        for lag in 1..=self.max_lag + 1 {
            let mut difference = 0.0;
            for i in 0..WINDOW {
                let a = self.history[(self.cursor + HISTORY - WINDOW + i) & (HISTORY - 1)];
                let b = self.history[(self.cursor + HISTORY - WINDOW + i - lag) & (HISTORY - 1)];
                let delta = a - b;
                difference += delta * delta;
            }
            cumulative += difference;
            let normalized = if cumulative > 0.0 {
                difference * lag as f32 / cumulative
            } else {
                1.0
            };

            if let Some((candidate_lag, before, at)) = candidate {
                if normalized >= at {
                    best = (candidate_lag, before, at, normalized);
                    break;
                }
            }
            if lag >= self.min_lag && normalized < YIN_THRESHOLD && normalized < previous {
                candidate = Some((lag, previous, normalized));
            }
            previous = normalized;
        }

        if best.0 == 0 {
            self.frequency = 0.0;
            return;
        }
        let (lag, before, at, after) = best;
        let denominator = before - 2.0 * at + after;
        let offset = if denominator.abs() > 1e-8 {
            (0.5 * (before - after) / denominator).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let frequency = self.rate / (lag as f32 + offset);
        self.frequency = if (MIN_HZ..=MAX_HZ).contains(&frequency) {
            frequency
        } else {
            0.0
        };
    }
}

#[cfg(test)]
mod tests {
    use super::FrequencyDetector;
    use core::f32::consts::TAU;

    fn feed_tone(detector: &mut FrequencyDetector, sample_rate: f32, hz: f32, harmonics: bool) {
        for i in 0..(sample_rate * 0.15) as usize {
            let phase = TAU * hz * i as f32 / sample_rate;
            let sample = if harmonics {
                0.12 * phase.sin() + 0.25 * (2.0 * phase).sin() + 0.18 * (3.0 * phase).sin()
            } else {
                0.5 * phase.sin()
            };
            detector.push(sample);
        }
    }

    #[test]
    fn tracks_guitar_range_at_common_rates() {
        for rate in [44_100.0, 48_000.0, 96_000.0] {
            for hz in [82.4069, 110.0, 440.0, 987.7666] {
                let mut detector = FrequencyDetector::new(rate).unwrap();
                feed_tone(&mut detector, rate, hz, false);
                let error = (detector.frequency_hz() - hz).abs() / hz;
                assert!(
                    error < 0.015,
                    "rate={rate}, hz={hz}, detected={}",
                    detector.frequency_hz()
                );
            }
        }
    }

    #[test]
    fn tracks_missing_fundamental() {
        let mut detector = FrequencyDetector::new(48_000.0).unwrap();
        feed_tone(&mut detector, 48_000.0, 110.0, true);
        assert!(
            (detector.frequency_hz() - 110.0).abs() < 2.0,
            "detected={}",
            detector.frequency_hz()
        );
    }

    #[test]
    fn silence_and_invalid_input_clear_frequency() {
        let mut detector = FrequencyDetector::new(48_000.0).unwrap();
        feed_tone(&mut detector, 48_000.0, 220.0, false);
        assert!(detector.frequency_hz() > 0.0);
        for _ in 0..4_000 {
            detector.push(f32::NAN);
        }
        assert_eq!(detector.frequency_hz(), 0.0);
        detector.reset();
        assert_eq!(detector.frequency_hz(), 0.0);
    }
}
