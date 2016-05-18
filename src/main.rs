#![allow(dead_code, unused_variables)]
extern crate xcb;
extern crate time;

use std::thread;
use xcb::randr;

mod solar;

/**
 * Constants
 */
const RANDR_MAJOR_VERSION: u32 = 1;
const RANDR_MINOR_VERSION: u32 = 3;
const NEUTRAL_TEMP:        f64 = 6500.0;
const DEFAULT_DAY_TEMP:    f64 = 5500.0;
const DEFAULT_NIGHT_TEMP:  f64 = 3500.0;
const DEFAULT_BRIGHTNESS:  f64 = 1.0;
const DEFAULT_GAMMA:       f64 = 1.0;

/**
 * Latitude and longitude location
 */
struct Location {
    lat: f64,
    lon: f64
}

/**
 * A color setting
 */
struct ColorSetting {
    temp: f64,
    gamma: [f64; 3],
    brightness: f64,
}

impl ColorSetting {
    fn new() -> ColorSetting {
        ColorSetting {
            temp: -1.0,
            gamma: [std::f64::NAN, std::f64::NAN, std::f64::NAN],
            brightness: std::f64::NAN
        }
    }
}

struct TransitionScheme {
    high: f64,
    low: f64,
    day: ColorSetting,
    night: ColorSetting
}

impl TransitionScheme {
    fn new() -> TransitionScheme {
        TransitionScheme {
            high: 3.0,
            low: solar::CIVIL_TWILIGHT_ELEV,
            day: ColorSetting::new(),
            night: ColorSetting::new()
        }
    }
}

struct Crtc {
    id: u32,
    ramp_size: u32,
    saved_ramps: (Vec<u16>, Vec<u16>, Vec<u16>)
}

/**
 * Wrapping struct for RandR state
 */
struct RandrState {
    conn: xcb::Connection,
    screen_num: i32,
    window_dummy: u32,
    crtcs: Vec<Crtc>
}

/**
 * Ensure mid is at least lo, and at most up
 */
#[inline]
fn clamp(lo: f64, mid: f64, up: f64) -> f64 {
    if mid < lo {
        lo
    } else if mid > up {
        up
    } else {
        mid
    }
}

fn interpolate_color_settings(elevation: f64, trans: &TransitionScheme) -> ColorSetting {
    let day = &trans.day;
    let night = &trans.night;

    let alpha = clamp(0.0, (trans.low - elevation) / (trans.low - trans.high), 1.0);

    let mut res = ColorSetting::new();
    res.temp = (1.0-alpha) * night.temp + alpha * day.temp;
    res.brightness = (1.0-alpha) * night.brightness + alpha * day.brightness;
    for i in 0..3 {
        res.gamma[i] = (1.0-alpha) * night.gamma[i] + alpha*day.gamma[i];
    }
    res
}

fn main() {
    let mut randr_state = RandrState::init();

    randr_state.query_version();
    randr_state.start();

    /* Run continual mode */
    // Location
    // Transition scheme
    // Gamma method
    // Gamma state (RandR)
    // transition: int
    // verbose: bool

    /* Init transition scheme - all defaults for now */
    let mut scheme = TransitionScheme::new();
    scheme.day.temp = DEFAULT_DAY_TEMP;
    scheme.night.temp = DEFAULT_NIGHT_TEMP;
    if scheme.day.brightness.is_nan() {
        scheme.day.brightness = DEFAULT_BRIGHTNESS;
    }
    if scheme.night.brightness.is_nan() {
        scheme.night.brightness = DEFAULT_BRIGHTNESS;
    }

    if scheme.day.gamma[0].is_nan() {
        for g in scheme.day.gamma.iter_mut() {
            *g = DEFAULT_GAMMA;
        }
    }
    if scheme.night.gamma[0].is_nan() {
        for g in scheme.night.gamma.iter_mut() {
            *g = DEFAULT_GAMMA;
        }
    }

    let mut now;
    loop {
        now = systemtime_get_time(); //::precise_time_s();
        println!("Adjusting at {:?}", now);

        // Compute elevation

        // Interpolate color settings: ColorSetting

        let lat = 55.0;
        let lon = 12.0;

        let elev = solar::elevation(now, lat, lon);
        println!("Current angular elevation of the sun: {:?}", elev);

        // Ongoing short transition?

        // Interpolate between 6500K and calculated temperature
        let color_setting = interpolate_color_settings(elev, &scheme);
        println!("Color temperature: {:?}K", color_setting.temp);
        println!("Brightness: {:?}", color_setting.brightness);

        // randr_state.set_temperature(&color_setting)


        // Sleep for 5 seconds or 0.1 second
        thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn systemtime_get_time() -> f64 {
    let now = time::get_time();
    now.sec as f64 + (now.nsec as f64 / 1000000.0)
}


/**
 *
 */
impl RandrState {
    fn set_temperature(&mut self, setting: &ColorSetting) {

    }

    /**
     *
     */
    fn start(&mut self) {
        let setup = self.conn.get_setup();
        let screen = setup.roots().nth(self.screen_num as usize).unwrap();

        /* Get list of CRTCs for the screen */
        let screen_resources = randr::get_screen_resources(&self.conn,
                                                           self.window_dummy).get_reply().unwrap();
        println!("Num CRTCs: {}", screen_resources.num_crtcs());
        let num_crtcs = screen_resources.num_crtcs();

        self.crtcs = Vec::with_capacity(screen_resources.num_crtcs() as usize);

        /* Save size and gamma ramps of all CRTCs */
        for crtc in screen_resources.crtcs() {
            let gamma = randr::get_crtc_gamma(&self.conn, *crtc).get_reply().unwrap();
            let red = gamma.red().to_vec();
            let green = gamma.green().to_vec();
            let blue = gamma.blue().to_vec();

            self.crtcs.push(Crtc {
                id: *crtc,
                ramp_size: gamma.size() as u32,
                saved_ramps: (red, green, blue)
            });
        }
    }

    fn init() -> RandrState {
        let (conn, screen_num) = xcb::Connection::connect(None).unwrap();

        let window_dummy = {
            let setup = conn.get_setup();
            let screen = setup.roots().nth(screen_num as usize).unwrap();
            let window_dummy = conn.generate_id();

            xcb::create_window(&conn, 0, window_dummy, screen.root(), 0, 0, 1,
                               1, 0, 0, 0, &[]);
            conn.flush();
            window_dummy
        };

        RandrState {
            conn: conn,
            screen_num: screen_num,
            window_dummy: window_dummy,
            crtcs: vec![]
        }
    }

    fn query_version(&self) {
        let reply = randr::query_version(&self.conn,
                                         RANDR_MAJOR_VERSION,
                                         RANDR_MINOR_VERSION).get_reply().unwrap();
        println!("RandR {}.{}", reply.major_version(),
                 reply.minor_version());
    }
}
