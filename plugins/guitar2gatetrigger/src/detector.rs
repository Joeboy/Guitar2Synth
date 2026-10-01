//! Sample-rate envelope, gate, and pluck-onset detector.

pub struct GateTriggerDetector {
    fast: f32,
    slow: f32,
    fast_attack: f32,
    fast_release: f32,
    slow_coefficient: f32,
    threshold: f32,
    gate: bool,
    onset_armed: bool,
    pulse_left: u32,
    refractory_left: u32,
    pulse_samples: u32,
    refractory_samples: u32,
}

impl GateTriggerDetector {
    pub fn new(sample_rate: f32) -> Option<Self> {
        if !sample_rate.is_finite() || !(8_000.0..=384_000.0).contains(&sample_rate) {
            return None;
        }
        Some(Self {
            fast: 0.0,
            slow: 0.0,
            fast_attack: coefficient(sample_rate, 0.001),
            fast_release: coefficient(sample_rate, 0.008),
            slow_coefficient: coefficient(sample_rate, 0.040),
            threshold: 0.01,
            gate: false,
            onset_armed: true,
            pulse_left: 0,
            refractory_left: 0,
            pulse_samples: (sample_rate * 0.005) as u32,
            refractory_samples: (sample_rate * 0.040) as u32,
        })
    }

    pub fn reset(&mut self) {
        self.fast = 0.0;
        self.slow = 0.0;
        self.gate = false;
        self.onset_armed = true;
        self.pulse_left = 0;
        self.refractory_left = 0;
    }

    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = if threshold.is_finite() {
            threshold.clamp(0.0005, 0.2)
        } else {
            0.01
        };
    }

    /// Returns (gate, trigger), each in the range 0–1.
    pub fn push(&mut self, input: f32) -> (f32, f32) {
        let level = if input.is_finite() { input.abs() } else { 0.0 };
        let threshold = self.threshold;

        let fast_coefficient = if level > self.fast {
            self.fast_attack
        } else {
            self.fast_release
        };
        self.fast += fast_coefficient * (level - self.fast);
        self.slow += self.slow_coefficient * (self.fast - self.slow);
        if self.fast < 1e-12 {
            self.fast = 0.0;
        }
        if self.slow < 1e-12 {
            self.slow = 0.0;
        }

        if self.refractory_left > 0 {
            self.refractory_left -= 1;
        }

        if self.gate {
            if self.fast < threshold * 0.5 {
                self.gate = false;
                self.onset_armed = true;
            } else if self.fast < self.slow * 1.25 {
                // A sustained note has settled enough to detect a new pluck.
                self.onset_armed = true;
            } else if self.onset_armed
                && self.refractory_left == 0
                && self.fast > self.slow * 1.8
                && self.fast > threshold * 1.5
            {
                self.trigger();
            }
        } else if self.fast >= threshold {
            self.gate = true;
            self.trigger();
        }

        let pulse = if self.pulse_left > 0 {
            self.pulse_left -= 1;
            1.0
        } else {
            0.0
        };
        (if self.gate { 1.0 } else { 0.0 }, pulse)
    }

    fn trigger(&mut self) {
        self.onset_armed = false;
        self.pulse_left = self.pulse_samples;
        self.refractory_left = self.refractory_samples;
    }
}

fn coefficient(sample_rate: f32, seconds: f32) -> f32 {
    1.0 / (1.0 + seconds * sample_rate)
}

#[cfg(test)]
mod tests {
    use super::GateTriggerDetector;

    #[test]
    fn gate_and_trigger_follow_note_and_silence() {
        let rate = 48_000.0;
        let mut detector = GateTriggerDetector::new(rate).unwrap();
        let mut trigger_rises = 0;
        let mut last_trigger = 0.0;
        let mut gate = 0.0;
        for i in 0..(rate * 0.3) as usize {
            let sample = 0.3 * (core::f32::consts::TAU * 110.0 * i as f32 / rate).sin();
            let (g, t) = detector.push(sample);
            gate = g;
            if t > 0.0 && last_trigger == 0.0 {
                trigger_rises += 1;
            }
            last_trigger = t;
        }
        assert_eq!(gate, 1.0);
        assert_eq!(trigger_rises, 1, "steady low E must not retrigger");
        for _ in 0..(rate * 0.1) as usize {
            (gate, _) = detector.push(0.0);
        }
        assert_eq!(gate, 0.0);
    }

    #[test]
    fn stronger_pluck_retriggers_while_gate_stays_high() {
        let rate = 48_000.0;
        let mut detector = GateTriggerDetector::new(rate).unwrap();
        let mut trigger_rises = 0;
        let mut last_trigger = 0.0;
        let mut gate_went_low = false;
        for i in 0..(rate * 0.35) as usize {
            let amplitude = if i < (rate * 0.20) as usize {
                0.12
            } else {
                0.45
            };
            let sample = amplitude * (core::f32::consts::TAU * 110.0 * i as f32 / rate).sin();
            let (gate, trigger) = detector.push(sample);
            if i > (rate * 0.03) as usize && gate == 0.0 {
                gate_went_low = true;
            }
            if trigger > 0.0 && last_trigger == 0.0 {
                trigger_rises += 1;
            }
            last_trigger = trigger;
        }
        assert!(!gate_went_low);
        assert_eq!(trigger_rises, 2);
    }

    #[test]
    fn harmonic_low_note_does_not_fire_on_each_cycle() {
        let rate = 48_000.0;
        let mut detector = GateTriggerDetector::new(rate).unwrap();
        let mut trigger_rises = 0;
        let mut last_trigger = 0.0;
        for i in 0..(rate * 0.35) as usize {
            let phase = core::f32::consts::TAU * 82.4 * i as f32 / rate;
            let sample =
                0.14 * phase.sin() + 0.09 * (2.0 * phase).sin() + 0.06 * (3.0 * phase).sin();
            let (_, trigger) = detector.push(sample);
            if trigger > 0.0 && last_trigger == 0.0 {
                trigger_rises += 1;
            }
            last_trigger = trigger;
        }
        assert_eq!(trigger_rises, 1);
    }

    #[test]
    fn threshold_and_invalid_input() {
        let mut detector = GateTriggerDetector::new(48_000.0).unwrap();
        detector.set_threshold(f32::NAN);
        for _ in 0..4_800 {
            assert_eq!(detector.push(f32::NAN), (0.0, 0.0));
        }
        detector.set_threshold(0.02);
        for i in 0..4_800 {
            let sample = 0.005 * (core::f32::consts::TAU * 220.0 * i as f32 / 48_000.0).sin();
            let (gate, _) = detector.push(sample);
            assert_eq!(gate, 0.0);
        }
        detector.reset();
        assert_eq!(detector.push(0.0), (0.0, 0.0));
    }
}
