use solar;

/* Periods of day */
pub enum Period {
    None,
    Day,
    Night,
    Transition
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

pub struct TransitionScheme {
    pub high: f64,
    pub low: f64,
    pub day: ColorSetting,
    pub night: ColorSetting
}

impl TransitionScheme {
    pub fn new() -> TransitionScheme {
        TransitionScheme {
            high:  3.0,
            low:   solar::CIVIL_TWILIGHT_ELEV,
            day:   ColorSetting::new(),
            night: ColorSetting::new()
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
}
