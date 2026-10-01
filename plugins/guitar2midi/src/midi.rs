use crate::frequency::FrequencyDetector;
use crate::gate_trigger::GateTriggerDetector;

const VELOCITY: u8 = 100;
const SEMITONE_RATIO: f32 = 1.059_463_1;
const HALF_SEMITONE_RATIO: f32 = 1.029_302_2;

pub struct MidiTracker {
    frequency: FrequencyDetector,
    gate: GateTriggerDetector,
    active_note: Option<u8>,
    need_note_off: bool,
    previous_trigger: bool,
    settle_left: u32,
    settle_samples: u32,
    candidate: Option<u8>,
    candidate_count: u8,
}

impl MidiTracker {
    pub fn new(sample_rate: f32) -> Option<Self> {
        Some(Self {
            frequency: FrequencyDetector::new(sample_rate)?,
            gate: GateTriggerDetector::new(sample_rate)?,
            active_note: None,
            need_note_off: false,
            previous_trigger: false,
            settle_left: 0,
            settle_samples: (sample_rate * 0.032) as u32,
            candidate: None,
            candidate_count: 0,
        })
    }

    pub fn reset(&mut self) {
        self.frequency.reset();
        self.gate.reset();
        self.active_note = None;
        self.need_note_off = false;
        self.previous_trigger = false;
        self.settle_left = 0;
        self.candidate = None;
        self.candidate_count = 0;
    }

    pub fn set_threshold(&mut self, threshold: f32) {
        self.gate.set_threshold(threshold);
    }

    /// `send` writes a three-byte MIDI event at the current sample and returns
    /// false when the host's Atom Sequence has no room for another event.
    pub fn push(&mut self, sample: f32, mut send: impl FnMut([u8; 3]) -> bool) {
        let estimate_ready = self.frequency.push(sample);
        let (gate, trigger) = self.gate.push(sample);
        let gate_high = gate > 0.0;
        let trigger_high = trigger > 0.0;

        if trigger_high && !self.previous_trigger {
            self.settle_left = self.settle_samples;
            self.candidate = None;
            self.candidate_count = 0;
            self.need_note_off = self.active_note.is_some();
        }
        self.previous_trigger = trigger_high;

        if !gate_high {
            self.settle_left = 0;
            self.candidate = None;
            self.candidate_count = 0;
            self.need_note_off = self.active_note.is_some();
        }

        if self.need_note_off {
            if let Some(note) = self.active_note {
                if send([0x80, note, 0]) {
                    self.active_note = None;
                    self.need_note_off = false;
                }
            } else {
                self.need_note_off = false;
            }
        }
        if !gate_high || self.need_note_off {
            return;
        }
        if self.settle_left > 0 {
            self.settle_left -= 1;
            return;
        }
        if !estimate_ready {
            return;
        }

        let Some(note) = frequency_to_note(self.frequency.frequency_hz()) else {
            self.candidate = None;
            self.candidate_count = 0;
            return;
        };
        if self.active_note == Some(note) {
            self.candidate = None;
            self.candidate_count = 0;
            return;
        }

        // First note after an onset starts immediately. Legato note changes
        // require two estimates to reject brief frequency tracking errors.
        if self.active_note.is_some() {
            if self.candidate == Some(note) {
                self.candidate_count = self.candidate_count.saturating_add(1);
            } else {
                self.candidate = Some(note);
                self.candidate_count = 1;
            }
            if self.candidate_count < 2 {
                return;
            }
            let old_note = self.active_note.unwrap();
            if !send([0x80, old_note, 0]) {
                return;
            }
            self.active_note = None;
        }

        if send([0x90, note, VELOCITY]) {
            self.active_note = Some(note);
            self.candidate = None;
            self.candidate_count = 0;
        }
    }
}

fn frequency_to_note(frequency: f32) -> Option<u8> {
    if !frequency.is_finite() || !(70.0..=1_300.0).contains(&frequency) {
        return None;
    }
    let mut note = 69i16;
    let mut center = 440.0f32;
    while frequency >= center * HALF_SEMITONE_RATIO && note < 127 {
        center *= SEMITONE_RATIO;
        note += 1;
    }
    while frequency < center / HALF_SEMITONE_RATIO && note > 0 {
        center /= SEMITONE_RATIO;
        note -= 1;
    }
    Some(note as u8)
}

#[cfg(test)]
mod tests {
    use super::{frequency_to_note, MidiTracker};

    #[test]
    fn standard_guitar_notes_are_quantized() {
        assert_eq!(frequency_to_note(82.4069), Some(40));
        assert_eq!(frequency_to_note(110.0), Some(45));
        assert_eq!(frequency_to_note(440.0), Some(69));
        assert_eq!(frequency_to_note(0.0), None);
    }

    #[test]
    fn note_on_then_off_for_one_pluck() {
        let rate = 48_000.0;
        let mut tracker = MidiTracker::new(rate).unwrap();
        let mut events = std::vec::Vec::new();
        for i in 0..(rate * 0.35) as usize {
            let sample = if i < (rate * 0.20) as usize {
                0.4 * (core::f32::consts::TAU * 82.4069 * i as f32 / rate).sin()
            } else {
                0.0
            };
            tracker.push(sample, |message| {
                events.push((i, message));
                true
            });
        }
        assert_eq!(events.len(), 2, "events={events:?}");
        assert_eq!(events[0].1, [0x90, 40, 100]);
        assert_eq!(events[1].1, [0x80, 40, 0]);
        assert!(events[0].0 < events[1].0);
    }

    #[test]
    fn new_pluck_releases_old_note_before_new_one() {
        let rate = 48_000.0;
        let mut tracker = MidiTracker::new(rate).unwrap();
        let mut events = std::vec::Vec::new();
        for i in 0..(rate * 0.55) as usize {
            let (amplitude, hz) = if i < (rate * 0.25) as usize {
                (0.12, 110.0)
            } else {
                (0.45, 146.8324)
            };
            let sample = amplitude * (core::f32::consts::TAU * hz * i as f32 / rate).sin();
            tracker.push(sample, |message| {
                events.push(message);
                true
            });
        }
        assert!(events.len() >= 3, "events={events:?}");
        assert_eq!(events[0], [0x90, 45, 100]);
        assert_eq!(events[1], [0x80, 45, 0]);
        assert_eq!(events[2], [0x90, 50, 100]);
    }
}
