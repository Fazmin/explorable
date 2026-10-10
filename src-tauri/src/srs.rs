//! Spaced repetition: an SM-2 variant with four ratings, the scheme most
//! students know from Anki. Intervals are in days; a failed card returns after
//! a short relearning step.

use chrono::{DateTime, Duration, Utc};

pub const AGAIN: i64 = 1;
pub const HARD: i64 = 2;
pub const GOOD: i64 = 3;
pub const EASY: i64 = 4;

const MIN_EASE: f64 = 1.3;
const MAX_EASE: f64 = 3.2;
/// Relearning step after "Again": ten minutes.
const RELEARN_DAYS: f64 = 10.0 / (24.0 * 60.0);
const MAX_INTERVAL_DAYS: f64 = 3650.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardState {
    pub ease: f64,
    pub interval_days: f64,
    pub reps: i64,
    pub lapses: i64,
}

impl Default for CardState {
    fn default() -> Self {
        CardState {
            ease: 2.5,
            interval_days: 0.0,
            reps: 0,
            lapses: 0,
        }
    }
}

/// Applies a rating and returns the new state. `rating` is clamped to 1..=4.
pub fn schedule(state: CardState, rating: i64) -> CardState {
    let rating = rating.clamp(AGAIN, EASY);
    let mut next = state;
    if rating == AGAIN {
        if state.reps > 0 {
            next.lapses += 1;
        }
        next.reps = 0;
        next.ease = (state.ease - 0.2).max(MIN_EASE);
        next.interval_days = RELEARN_DAYS;
        return next;
    }

    next.ease = match rating {
        HARD => state.ease - 0.15,
        EASY => state.ease + 0.15,
        _ => state.ease,
    }
    .clamp(MIN_EASE, MAX_EASE);

    next.interval_days = match state.reps {
        0 => match rating {
            HARD => 1.0,
            GOOD => 1.0,
            _ => 4.0,
        },
        1 => match rating {
            HARD => 3.0,
            GOOD => 6.0,
            _ => 8.0,
        },
        _ => {
            let base = state.interval_days.max(1.0);
            match rating {
                HARD => base * 1.2,
                GOOD => base * next.ease,
                _ => base * next.ease * 1.3,
            }
        }
    };
    // A pass never schedules sooner than the previous interval (except Hard on
    // young cards, which already uses fixed steps).
    if state.reps >= 2 && rating != HARD {
        next.interval_days = next.interval_days.max(state.interval_days + 1.0);
    }
    next.interval_days = next.interval_days.min(MAX_INTERVAL_DAYS);
    next.reps = state.reps + 1;
    next
}

pub fn due_after(now: DateTime<Utc>, interval_days: f64) -> DateTime<Utc> {
    let seconds = (interval_days * 86_400.0).round() as i64;
    now + Duration::seconds(seconds.max(60))
}

/// A compact label such as "10m", "1d", "3.5mo" for an interval in days.
pub fn interval_label(days: f64) -> String {
    let minutes = days * 24.0 * 60.0;
    if minutes < 60.0 {
        format!("{}m", minutes.round().max(1.0) as i64)
    } else if days < 1.0 {
        format!("{}h", (minutes / 60.0).round() as i64)
    } else if days < 30.0 {
        format!("{}d", days.round() as i64)
    } else if days < 365.0 {
        format_trim(days / 30.0, "mo")
    } else {
        format_trim(days / 365.0, "y")
    }
}

fn format_trim(value: f64, unit: &str) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if (rounded - rounded.round()).abs() < f64::EPSILON {
        format!("{}{unit}", rounded as i64)
    } else {
        format!("{rounded:.1}{unit}")
    }
}

/// Labels for what each of the four ratings would schedule.
pub fn previews(state: CardState) -> [String; 4] {
    [AGAIN, HARD, GOOD, EASY].map(|r| interval_label(schedule(state, r).interval_days))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_card_steps() {
        let s = CardState::default();
        assert_eq!(schedule(s, GOOD).interval_days, 1.0);
        assert_eq!(schedule(s, EASY).interval_days, 4.0);
        assert!(schedule(s, AGAIN).interval_days < 0.01);
        assert_eq!(schedule(s, GOOD).reps, 1);
    }

    #[test]
    fn intervals_grow_with_good_ratings() {
        let mut s = CardState::default();
        let mut last = 0.0;
        for _ in 0..6 {
            s = schedule(s, GOOD);
            assert!(s.interval_days > last, "{} should exceed {}", s.interval_days, last);
            last = s.interval_days;
        }
        assert!(last > 60.0);
    }

    #[test]
    fn again_resets_and_counts_lapse() {
        let mut s = CardState::default();
        s = schedule(s, GOOD);
        s = schedule(s, GOOD);
        let failed = schedule(s, AGAIN);
        assert_eq!(failed.reps, 0);
        assert_eq!(failed.lapses, 1);
        assert!(failed.ease < s.ease);
        // Failing a never-learned card is not a lapse.
        assert_eq!(schedule(CardState::default(), AGAIN).lapses, 0);
    }

    #[test]
    fn ease_is_bounded() {
        let mut s = CardState::default();
        for _ in 0..30 {
            s = schedule(s, AGAIN);
        }
        assert!((s.ease - MIN_EASE).abs() < 1e-9);
        let mut e = CardState::default();
        for _ in 0..30 {
            e = schedule(e, EASY);
        }
        assert!(e.ease <= MAX_EASE + 1e-9);
        assert!(e.interval_days <= MAX_INTERVAL_DAYS);
    }

    #[test]
    fn rating_order_is_monotonic() {
        let mut s = CardState::default();
        s = schedule(s, GOOD);
        s = schedule(s, GOOD);
        let iv: Vec<f64> = [AGAIN, HARD, GOOD, EASY]
            .iter()
            .map(|&r| schedule(s, r).interval_days)
            .collect();
        assert!(iv.windows(2).all(|w| w[0] < w[1]), "{iv:?}");
    }

    #[test]
    fn labels() {
        assert_eq!(interval_label(RELEARN_DAYS), "10m");
        assert_eq!(interval_label(0.5), "12h");
        assert_eq!(interval_label(4.0), "4d");
        assert_eq!(interval_label(90.0), "3mo");
        assert_eq!(interval_label(105.0), "3.5mo");
        assert_eq!(interval_label(730.0), "2y");
    }
}
