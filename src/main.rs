extern crate xcb;
extern crate time;
#[macro_use]
extern crate chan;
extern crate chan_signal;

use std::thread;
use gamma_method::GammaMethodProvider;

mod transition;
mod colorramp;
mod location;
mod solar;
mod gamma_method;
mod gamma_randr;


/**
 * Constants
 */
const NEUTRAL_TEMP:        i32 = 6500;
const DEFAULT_DAY_TEMP:    i32 = 5500;
const DEFAULT_NIGHT_TEMP:  i32 = 3500;
const DEFAULT_BRIGHTNESS:  f64 = 1.0;
const DEFAULT_GAMMA:       f64 = 1.0;

// TODO Use docopt?
const USAGE: &'static str = "
redshift-rs: A Rust clone of RedShift

Usage:
  redshift-rs [options]
  redshift-rs (-h | --help)
  redshift-rs --version

Options:
  -h, --help     Display this help message
  -V, --version  Print version and exit
  -v, --verbose  Verbose output

  -b DAY:NIGHT   Screen brightness to apply (between 0.1 and 1.0)
  -l LAT:LON     Use this location (latitude and longitude)
  -m METHOD      Method to use to set color temperature
                 (use 'list' to see available providers)
  -o             One shot mode
  -O TEMP        One shot manual mode (set color temperature)
  -p             Print parameters and exit
  -x             Reset (remove adjustments to screen)
  -r             Disable temperature transitions
  -t DAY:NIGHT   Set day/night color temperatures
";

// static gamma_methods: &'static [Box<GammaMethodProvider>] = &[
//     #[cfg(unix)]
//     Box::new(gamma_randr::RandrMethod) as Box<GammaMethodProvider>,

//     #[cfg(unix)]
//     Box::new(gamma_method::DummyMethod) as Box<GammaMethodProvider>
// ];

fn main() {
    let mut gamma_state = gamma_randr::RandrMethod.init();
    //let mut gamma_state = gamma_method::DummyMethod.init();

     gamma_state.start();

    /* Run continual mode */
 
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

    /* Init location */
    let loc = location::Location::new(55.7, 12.6);

    // Create signal thread
    let sigint = chan_signal::notify(&[chan_signal::Signal::INT,
                                       chan_signal::Signal::TERM]);
    let (signal_tx, signal_rx) = chan::sync(0);
    std::thread::spawn(move || {
        for sig in sigint.iter() {
            signal_tx.send(sig);
        }
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
    let mut exiting = false;
    let mut prev_color_setting = transition::ColorSetting::new();
    let mut prev_period = transition::Period::None;
    sleep_tx.send(TimerMsg::Sleep(0));
    loop {
        chan_select! {
            signal_rx.recv() -> _signal => {
                if exiting {
                    break // If already exiting, just exit immediately
                }
                exiting = true;
                scheme.short_trans_delta = 1;
                scheme.short_trans_len = 2;
                scheme.adjustment_alpha = 0.1;
            },
            timer_rx.recv() => {
                now = systemtime_get_time();

                // Compute elevation
                let elev = solar::elevation(now, &loc);

                let period = scheme.get_period(elev);
                if period != prev_period {
                    period.print();
                    prev_period = period;
                }

                // Interpolate between 6500K and calculated temperature
                let mut color_setting = scheme.interpolate_color_settings(elev);

                /* Ongoing short transition? */
                if scheme.short_transition() {
                    scheme.adjust_transition_alpha();
                    color_setting.temp = (scheme.adjustment_alpha * NEUTRAL_TEMP as f64 +
                                          (1.0-scheme.adjustment_alpha) * color_setting.temp as f64) as i32;
                    color_setting.brightness = scheme.adjustment_alpha * 1.0 +
                        (1.0-scheme.adjustment_alpha) * color_setting.brightness;
                }

                if color_setting != prev_color_setting {
                    if color_setting.temp != prev_color_setting.temp {
                        println!("Color temperature: {:?}K", color_setting.temp);
                    }
                    if color_setting.brightness != prev_color_setting.brightness {
                        println!("Brightness: {:?}", color_setting.brightness);
                    }
                    gamma_state.set_temperature(&color_setting);
                }

                if exiting && !scheme.short_transition() {
                    break
                }

                // Sleep for 5 seconds or 0.1 second
                sleep_tx.send(TimerMsg::Sleep(if scheme.short_transition() { 100 } else { 5000 }));

                /* Save temperature */
                prev_color_setting = color_setting;
            }
        }
    }

    chan_select! {
        default => {},
        timer_rx.recv() => {}
    }
    sleep_tx.send(TimerMsg::Exit);

    gamma_state.restore();
}

fn systemtime_get_time() -> f64 {
    let now = time::get_time();
    now.sec as f64 + (now.nsec as f64 / 1_000_000_000.0)
}

