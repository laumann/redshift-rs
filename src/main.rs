#![allow(dead_code, unused_variables)]
extern crate xcb;
extern crate time;
#[macro_use]
extern crate chan;
extern crate chan_signal;

use std::thread;
use xcb::randr;

mod transition;
mod colorramp;
mod location;
mod solar;

/**
 * Constants
 */
const RANDR_MAJOR_VERSION: u32 = 1;
const RANDR_MINOR_VERSION: u32 = 3;
const NEUTRAL_TEMP:        i32 = 6500;
const DEFAULT_DAY_TEMP:    i32 = 5500;
const DEFAULT_NIGHT_TEMP:  i32 = 3500;
const DEFAULT_BRIGHTNESS:  f64 = 1.0;
const DEFAULT_GAMMA:       f64 = 1.0;

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

    /* Init transition scheme - all defaults for now */
    let mut scheme = transition::TransitionScheme::new();
    scheme.day.temp = DEFAULT_DAY_TEMP;
    scheme.night.temp = DEFAULT_NIGHT_TEMP;
    if scheme.day.brightness.is_nan() {
        scheme.day.brightness = DEFAULT_BRIGHTNESS;
    }
    if scheme.night.brightness.is_nan() {
        scheme.night.brightness = DEFAULT_BRIGHTNESS;
    }

    if scheme.day.gamma[0].is_nan() {
        for g in scheme.day.gamma.iter_mut() { *g = DEFAULT_GAMMA }
    }
    if scheme.night.gamma[0].is_nan() {
        for g in scheme.night.gamma.iter_mut() { *g = DEFAULT_GAMMA }
    }

    let loc = location::Location {
        lat: 55.7,
        lon: 12.6
    };

    // Create signal thread
    let sigint = chan_signal::notify(&[chan_signal::Signal::INT,
                                       chan_signal::Signal::TERM]);
    let (signal_tx, signal_rx) = chan::sync(0);
    std::thread::spawn(move || {
        signal_tx.send(sigint.recv());
    });

    // Create timer thread
    // The timer thread should be modifiable, to 
    enum TimerMsg {
        Sleep(u64),
        Exit
    }
    let (timer_tx, timer_rx) = chan::sync(0);
    let (sleep_tx, sleep_rx) = chan::sync(0);
    std::thread::spawn(move || {
        for msg in sleep_rx.iter() {
            match msg {
                TimerMsg::Sleep(ms) => {
                    thread::sleep(std::time::Duration::from_millis(ms));
                    timer_tx.send(());
                }
                TimerMsg::Exit => break
            }
        }
    });

    let mut now;
    let mut prev_color_setting = transition::ColorSetting::new();
    let mut prev_elev = 0.0;
    loop {
        now = systemtime_get_time();
        chan_select! {
            signal_rx.recv() -> signal => {
                println!("Received signal: {:?}", signal);
                break;
            },
            timer_rx.recv() => {}
        }


        // Compute elevation
        let elev = solar::elevation(now, &loc);

        /* Ongoing short transition? */

        // Interpolate between 6500K and calculated temperature
        let color_setting = scheme.interpolate_color_settings(elev);
        if color_setting.temp != prev_color_setting.temp {
            println!("Color temperature: {:?}K", color_setting.temp);
        }
        if color_setting.brightness != prev_color_setting.brightness {
            println!("Brightness: {:?}", color_setting.brightness);
        }

        
        // randr_state.set_temperature(&color_setting);

        // Sleep for 5 seconds or 0.1 second
        //thread::sleep(std::time::Duration::from_millis(100));
        //thread::sleep(std::time::Duration::from_secs(5));
        sleep_tx.send(TimerMsg::Sleep(100));


        /* Save temperature */
        prev_color_setting = color_setting;
    }
}

fn systemtime_get_time() -> f64 {
    let now = time::get_time();
    now.sec as f64 + (now.nsec as f64 / 1_000_000_000.0)
}

/**
 *
 */
impl RandrState {

    fn restore(&self) {
        for crtc in self.crtcs.iter() {
            randr::set_crtc_gamma_checked(&self.conn,
                                          crtc.id,
                                          &crtc.saved_ramps.0[..],
                                          &crtc.saved_ramps.1[..],
                                          &crtc.saved_ramps.2[..]);
        }
    }

    fn set_temperature(&self, setting: &transition::ColorSetting) {
        for crtc in self.crtcs.iter() {
            self.set_crtc_temperature(setting, crtc);
        }
    }

    fn set_crtc_temperature(&self, setting: &transition::ColorSetting, crtc: &Crtc) {
        //println!("CRTC[{:?}]", crtc.id);

        /* Borrow saved ramps from CRTC */
        let mut r = crtc.saved_ramps.0.clone();
        let mut g = crtc.saved_ramps.1.clone();
        let mut b = crtc.saved_ramps.2.clone();

        /* Create new gamma ramps */
        colorramp::colorramp_fill(&mut r[..], &mut g[..], &mut b[..],
                                  setting,
                                  crtc.ramp_size as usize);
        
        randr::set_crtc_gamma_checked(&self.conn,
                                      crtc.id,
                                      &r[..],
                                      &g[..],
                                      &b[..]);
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
