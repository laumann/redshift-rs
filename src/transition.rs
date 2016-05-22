use solar;

/* Periods of day */
#[derive(Debug, PartialEq)]
pub enum Period {
    None,
    Day,
    Night,
    Transition(f64)
}

impl Period {
    pub fn print(&self) {
        match *self {
            Period::None | Period::Day | Period::Night => {
                println!("Period: {:?}", *self);
            }
            Period::Transition(t) => {
                println!("Period: {} ({:.*}% day)", "Transition", 2, t * 100.0);
            }
        }
    }
}

/**
 * A color setting
 */
pub struct ColorSetting {
    pub temp: i32,
    pub gamma: [f64; 3],
    pub brightness: f64,
}

impl ColorSetting {
    pub fn new() -> ColorSetting {
        ColorSetting {
            temp: -1,
            gamma: [::std::f64::NAN,
                    ::std::f64::NAN,
                    ::std::f64::NAN],
            brightness: ::std::f64::NAN
        }
    }
}

/**
 * Transition scheme.
 * The solar elevations at which the transition begins/ends and
 * associated color settings.
 */
pub struct TransitionScheme {
    pub high: f64,
    pub low: f64,
    pub day: ColorSetting,
    pub night: ColorSetting,

    /* Used for initial and final gradual transition from/to 6500K */
    pub short_trans_delta: i16,
    pub short_trans_len: u16,
    pub adjustment_alpha: f64
}

impl TransitionScheme {
    pub fn new() -> TransitionScheme {
        TransitionScheme {
            high:  3.0,
            low:   solar::CIVIL_TWILIGHT_ELEV,
            day:   ColorSetting::new(),
            night: ColorSetting::new(),

            short_trans_delta: -1,
            short_trans_len: 10,
            adjustment_alpha: 1.0
        }
    }

    /**
     * Given an elevation, compute a color setting from this scheme's settings
     */
    pub fn interpolate_color_settings(&self, elevation: f64) -> ColorSetting {
        let day = &self.day;
        let night = &self.night;

        let al = (self.low - elevation) / (self.low - self.high);
        let alpha = al.min(1.0).max(0.0); // clamp to [0.0, 1.0]

        ColorSetting {
            temp: ((1.0-alpha) * night.temp as f64 + alpha * day.temp as f64) as i32,
            brightness: (1.0-alpha) * night.brightness + alpha * day.brightness,
            gamma: [
                (1.0-alpha) * night.gamma[0] + alpha*day.gamma[0],
                (1.0-alpha) * night.gamma[1] + alpha*day.gamma[1],
                (1.0-alpha) * night.gamma[2] + alpha*day.gamma[2]
            ]
        }
    }

    /* Determine how far through a transition progress we are. */
    pub fn transition_progress(&self, elevation: f64) -> f64 {
        if elevation < self.low {
            0.0
        } else if elevation > self.high {
            1.0
        } else {
            (self.low - elevation) / (self.low - self.high)
        }
    }

    pub fn get_period(&self, elevation: f64) -> Period {
        if elevation < self.low {
            Period::Night
        } else if elevation > self.high {
            Period::Day
        } else {
            let t = (self.low - elevation) / (self.low - self.high);
            Period::Transition(t)
        }
    }

    pub fn short_transition(&self) -> bool {
        self.short_trans_delta != 0
    }

    pub fn adjust_transition_alpha(&mut self) {
        self.adjustment_alpha += self.short_trans_delta as f64 * 0.1 / self.short_trans_len as f64;

        /* Stop transition when done */
        if self.adjustment_alpha <= 0.0 || self.adjustment_alpha >= 1.0 {
            self.short_trans_delta = 0;
        }

        /* Clamp alpha value */
        self.adjustment_alpha = self.adjustment_alpha.max(0.0).min(1.0);
    }
}
