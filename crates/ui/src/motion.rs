//! Fluent motion: the easing curves and durations Windows 11 animates with.

use std::time::{Duration, Instant};

use windows::{
    Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    },
    core::BOOL,
};

/// For small elements and quick feedback.
pub const FAST: Duration = Duration::from_millis(167);
/// For larger surfaces.
pub const NORMAL: Duration = Duration::from_millis(250);

/// A cubic Bézier curve from (0, 0) to (1, 1), like CSS `cubic-bezier()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Easing {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl Easing {
    /// Elements entering the screen: fast start, gentle stop.
    pub const DECELERATE: Self = Self::new(0.0, 0.0, 0.0, 1.0);
    /// Elements leaving the screen: gentle start, fast exit.
    pub const ACCELERATE: Self = Self::new(1.0, 0.0, 1.0, 1.0);
    /// Elements moving from one place to another on screen.
    pub const POINT_TO_POINT: Self = Self::new(0.55, 0.55, 0.0, 1.0);

    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Maps linear progress (0–1) to eased progress.
    pub fn apply(&self, progress: f32) -> f32 {
        let x = progress.clamp(0.0, 1.0);
        // The curve's x is monotonic, so bisection finds the parameter for `x`.
        let (mut low, mut high) = (0.0, 1.0);
        let mut t = x;
        for _ in 0..24 {
            let error = bezier(self.x1, self.x2, t) - x;
            if error.abs() < 1e-5 {
                break;
            }
            if error < 0.0 {
                low = t;
            } else {
                high = t;
            }
            t = (low + high) / 2.0;
        }
        bezier(self.y1, self.y2, t)
    }
}

/// One coordinate of a cubic Bézier with endpoints 0 and 1.
fn bezier(p1: f32, p2: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
}

/// A value moving from one number to another over time.
#[derive(Clone, Copy, Debug)]
pub struct Animation {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
    easing: Easing,
}

impl Animation {
    /// Starts now, or lands on `to` at once if animation effects are off in
    /// Windows' accessibility settings.
    pub fn new(from: f32, to: f32, duration: Duration, easing: Easing) -> Self {
        let duration = if animations_enabled() {
            duration
        } else {
            Duration::ZERO
        };
        Self::lasting(from, to, duration, easing)
    }

    /// Already at rest on `value`.
    pub fn idle(value: f32) -> Self {
        Self::lasting(value, value, Duration::ZERO, Easing::DECELERATE)
    }

    fn lasting(from: f32, to: f32, duration: Duration, easing: Easing) -> Self {
        Self {
            from,
            to,
            start: Instant::now(),
            duration,
            easing,
        }
    }

    /// Heads for `to` from wherever this animation is now, unless it already is.
    pub fn retarget(&self, to: f32, duration: Duration, easing: Easing) -> Self {
        if to == self.to {
            return *self;
        }
        Self::new(self.value(Instant::now()), to, duration, easing)
    }

    /// Where it's heading.
    pub fn target(&self) -> f32 {
        self.to
    }

    pub fn value(&self, now: Instant) -> f32 {
        let eased = self.easing.apply(self.progress(now));
        self.from + (self.to - self.from) * eased
    }

    pub fn is_running(&self, now: Instant) -> bool {
        self.progress(now) < 1.0
    }

    fn progress(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        let elapsed = now.saturating_duration_since(self.start);
        (elapsed.as_secs_f32() / self.duration.as_secs_f32()).min(1.0)
    }
}

/// Whether Windows' "Animation effects" setting is on. It's read each time, so a
/// change applies at once; reading it takes under a microsecond.
fn animations_enabled() -> bool {
    let mut enabled = BOOL(1);
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&raw mut enabled).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    read.is_err() || enabled.as_bool()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_keeps_its_endpoints() {
        for easing in [
            Easing::DECELERATE,
            Easing::ACCELERATE,
            Easing::POINT_TO_POINT,
        ] {
            assert!(easing.apply(0.0).abs() < 1e-4);
            assert!((easing.apply(1.0) - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn decelerate_is_ahead_and_accelerate_behind() {
        assert!(Easing::DECELERATE.apply(0.5) > 0.8);
        assert!(Easing::ACCELERATE.apply(0.5) < 0.2);
    }

    #[test]
    fn animation_runs_then_rests_on_target() {
        let animation = Animation::lasting(10.0, 20.0, FAST, Easing::DECELERATE);
        let start = animation.start;
        assert_eq!(animation.value(start), 10.0);
        assert!(animation.is_running(start + FAST / 2));
        assert_eq!(animation.value(start + FAST), 20.0);
        assert!(!animation.is_running(start + FAST));
    }

    #[test]
    fn idle_never_runs() {
        let animation = Animation::idle(3.0);
        assert_eq!(animation.value(Instant::now()), 3.0);
        assert!(!animation.is_running(Instant::now()));
    }
}
