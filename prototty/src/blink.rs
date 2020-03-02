use std::time::Duration;

pub struct Blink {
    cycle_length: Duration,
    min_alpha: u8,
    max_alpha: u8,
}

impl Blink {
    fn intensity(&self, duration: Duration) -> u8 {
        let cycle_length_micros = self.cycle_length.as_micros();
        let duration_micros = duration.as_micros();
        let progress_through_cycle_micros = duration_micros % cycle_length_micros;
        let scaled_progress = (progress_through_cycle_micros * 512) / cycle_length_micros;
        if scaled_progress < 256 {
            scaled_progress as u8
        } else {
            (511 - scaled_progress) as u8
        }
    }
    pub fn alpha(&self, duration: Duration) -> u8 {
        let intensity = self.intensity(duration);
        let delta = self.max_alpha - self.min_alpha;
        let offset = ((delta as u16 * intensity as u16) / 255 as u16) as u8;
        self.min_alpha + offset
    }
    pub fn new() -> Self {
        Self {
            cycle_length: Duration::from_millis(500),
            min_alpha: 64,
            max_alpha: 187,
        }
    }
}
