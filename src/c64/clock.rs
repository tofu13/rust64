// timing clock structure
extern crate time;

pub const SAMPLE_PERIOD: f64 = 1.0; // Update samples for throttling and benchmark [secs]

pub struct Clock {
    sample_time: f64,
}

impl Clock {
    pub fn new() -> Clock {
        let mut clock = Clock { sample_time: 0.0 };

        clock.sample_time = time::OffsetDateTime::now_utc().unix_timestamp_nanos() as f64 / 1E09;
        clock
    }

    pub fn sample(&mut self) -> Option<f64> {
        let current = time::OffsetDateTime::now_utc().unix_timestamp_nanos() as f64 / 1E09;
        let elapsed = current - self.sample_time;
        if elapsed >= SAMPLE_PERIOD {
            self.sample_time = current;
            return Some(elapsed);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_returns_true_and_updates_last_time_once_period_elapsed() {
        let mut clock = Clock::new();
        clock.sample_time = 0.0; // force "a long time ago"

        assert!(clock.sample().is_some());
        assert!(clock.sample_time > 0.0); // advanced to current time
    }

    #[test]
    fn sample_returns_false_before_period_elapses() {
        let mut clock = Clock::new();
        let far_future =
            time::OffsetDateTime::now_utc().unix_timestamp_nanos() as f64 / 1E09 + 1_000_000.0;
        clock.sample_time = far_future;

        assert!(clock.sample().is_none());
        assert_eq!(clock.sample_time, far_future); // unchanged
    }
}
