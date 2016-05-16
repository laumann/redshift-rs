#![allow(dead_code, unused_variables)]
extern crate xcb;
extern crate time;

use std::thread;
use xcb::randr;

mod solar;

struct Location {
    lat: f64,
    lon: f64
}

struct ColorSetting {
    temp: i32,
    gamma: [f64; 3],
    brightness: f64,
}

impl ColorSetting {
    fn new() -> ColorSetting {
        ColorSetting {
            temp: -1,
            gamma: [std::f64::NAN, std::f64::NAN, std::f64::NAN],
            brightness: std::f64::NAN
        }
    }
}

const RANDR_MAJOR_VERSION: u32 = 1;
const RANDR_MINOR_VERSION: u32 = 3;

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

    let mut now;
    loop {
        now = systemtime_get_time(); //::precise_time_s();
        println!("Adjusting at {:?}", now);
        
        // Compute elevation

        // Interpolate color settings: ColorSetting
        let mut color_setting = ColorSetting::new();
        
        let lat = 55.0;
        let lon = 12.0;

        let elev = solar::elevation(now, lat, lon);
        println!("Current angular elevation of the sun: {:?}", elev);

        // Ongoing short transition?

        // Interpolate between 6500K and calculated temperature

        // randr_state.set_temperature(&color_setting)
        
        
        // Sleep for 5 seconds or 0.1 second
        thread::sleep(std::time::Duration::from_secs(5));
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
