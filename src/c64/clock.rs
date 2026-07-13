// timing clock structure
extern crate time;

pub struct Clock {
    curr_time: f64,
    last_time: f64,
    clock_period: f64,
}

impl Clock {
    pub fn new(freq: f64) -> Clock {
        let mut clock = Clock {
            curr_time: 0.0,
            last_time: 0.0,
            clock_period: 1.0 / freq,
        };

        clock.last_time = time::OffsetDateTime::now_utc().unix_timestamp_nanos() as f64 / 1E09;
        clock
    }

    pub fn tick(&mut self) -> bool {
        self.curr_time = time::OffsetDateTime::now_utc().unix_timestamp_nanos() as f64 / 1E09;

        if self.curr_time - self.last_time >= self.clock_period {
            self.last_time = self.curr_time;
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_computes_clock_period_from_frequency() {
        let clock = Clock::new(2.0);
        assert_eq!(clock.clock_period, 0.5);
    }

    #[test]
    fn tick_returns_true_and_updates_last_time_once_period_elapsed() {
        let mut clock = Clock::new(1_000_000.0); // period = 1 microsecond
        clock.last_time = 0.0; // force "a long time ago"

        assert!(clock.tick());
        assert!(clock.last_time > 0.0); // advanced to current time
    }

    #[test]
    fn tick_returns_false_before_period_elapses() {
        let mut clock = Clock::new(1.0); // period = 1 second
        let far_future =
            time::OffsetDateTime::now_utc().unix_timestamp_nanos() as f64 / 1E09 + 1_000_000.0;
        clock.last_time = far_future;

        assert!(!clock.tick());
        assert_eq!(clock.last_time, far_future); // unchanged
    }
}
