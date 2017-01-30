//!
//! # Redshift in Rust
//!
//! aka redshift-rs
//! aka rustshift
//!
extern crate xcb;
extern crate time;
#[macro_use]
extern crate chan;
extern crate chan_signal;

extern crate docopt;
extern crate rustc_serialize;

use std::thread;
use gamma_method::GammaMethodProvider;
use docopt::Docopt;

mod transition;
mod colorramp;
mod location;
mod solar;
mod gamma_method;
mod gamma_randr;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

// Constants
const NEUTRAL_TEMP:        i32 = 6500;
const DEFAULT_DAY_TEMP:    i32 = 5500;
const DEFAULT_NIGHT_TEMP:  i32 = 3500;
const DEFAULT_BRIGHTNESS:  f64 = 1.0;
const DEFAULT_GAMMA:       f64 = 1.0;

const USAGE: &'static str = "
redshift-rs: A Rust clone of RedShift

Usage:
  redshift-rs [options]
  redshift-rs (-h | --help)
  redshift-rs --version

Options:
  -h, --help       Display this help message
  -V, --version    Print version and exit
  -v, --verbose    Verbose output

  -b <DAY:NIGHT>   Screen brightness to apply (between 0.1 and 1.0)
  -l <LAT:LON>     Use this location (latitude and longitude)
  -m <METHOD>      Method to use to set color temperature
                   (use 'list' to see available providers)
  -o               One shot mode
  -O <TEMP>        One shot manual mode (set color temperature)
  -p               Print parameters and exit
  -x               Reset (remove adjustments to screen)
  -r               Disable temperature transitions
  -t <DAY:NIGHT>   Set day/night color temperatures
";

// Error codes returned
#[derive(Debug)]
pub enum RedshiftError {
    MalformedArgument(String),
    Version,
    PrintMode
}

impl RedshiftError {

    /// Return whether this was a fatal error or not
    fn fatal(&self) -> bool {
        use RedshiftError::*;
        match *self {
            MalformedArgument(_) => true,
            Version | PrintMode => false
        }
    }

    /// Exit with an error code.
    /// The error code may not be fatal, in which case no error is printed.
    fn exit(&self) -> ! {
        let code = if self.fatal() {
            println!("{:?}", *self);
            1
        } else {
            0
        };
        ::std::process::exit(code);
    }
}

#[derive(RustcDecodable)]
struct Args {
    flag_version: bool,
    flag_verbose: bool,
    flag_b: Option<String>,
    flag_l: Option<String>,
    arg_m: Option<String>,
    flag_o: bool,
    arg_O: Option<String>,
    flag_p: bool,
    flag_x: bool,
    flag_r: bool,
    flag_t: Option<String>
}

#[inline]
fn malformed<T>(msg: String) -> Result<T, RedshiftError> {
    Err(RedshiftError::MalformedArgument(msg))
}

/// Parse the temperature argument
///
/// Expected as "DAY:NIGHT", where DAY and NIGHT are floating point
/// numbers. Any other input produces an error
fn parse_temperature(input: String) -> Result<(i32, i32), RedshiftError> {
    let mut parts = input.split(':');

    let day = parts.next()
        .map_or(malformed(format!("temperature argument: {}", input)),
                |l| l.parse().or(
                    malformed(format!("temperature argument: {} (of {})", l, input))))?;

    let night = parts.next()
        .map_or(malformed(format!("temperature argument: {}", input)),
                |l| l.parse().or(
                    malformed(format!("temperature argument: {} (of {})", l, input))))?;

    parts.next().map_or(Ok((day, night)),
                        |_| malformed(format!("temperature argument: {}", input)))
}

fn parse_brightness(input: String) -> Result<(f64, f64), RedshiftError> {
    let mut parts = input.split(':');

    let day = parts.next()
        .map_or(malformed(format!("brightness: {}", input)),
                |l| l.parse().or(
                    malformed(format!("brightness: {} (of {})", l, input))))?;

    let night = parts.next()
        .map_or(Ok(day),
                |l| l.parse().or(malformed(format!("brightness: {} (of {})", l, input))))?;

    parts.next()
        .map_or(Ok((day, night)),
                |trailing| malformed(format!("brightness: trailing {} (of {})", trailing, input)))
}

fn main() {

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.argv(std::env::args().into_iter()).decode())
        .unwrap_or_else(|e| e.exit());

    if args.flag_version {
        println!("redshift-rs {}", VERSION);
        RedshiftError::Version.exit();
    }

    let verbose = args.flag_verbose || args.flag_p;

    // Init location
    let loc = args.flag_l
        .map_or(location::Location::new(55.7, 12.6),
                |input| input.parse::<location::Location>().unwrap_or_else(|e| e.exit()));

    let (temp_day, temp_night) = args.flag_t
        .map_or((DEFAULT_DAY_TEMP, DEFAULT_NIGHT_TEMP),
                |input| parse_temperature(input).unwrap_or_else(|e| e.exit()));

    let (bright_day, bright_night) = args.flag_b
        .map_or((DEFAULT_BRIGHTNESS, DEFAULT_BRIGHTNESS),
                |input| parse_brightness(input).unwrap_or_else(|e| e.exit()));

    /* Init transition scheme */
    let mut scheme = transition::TransitionScheme::new();
    scheme.day.temp = temp_day;
    scheme.night.temp = temp_night;
    scheme.day.brightness = bright_day;
    scheme.night.brightness = bright_night;

    if verbose {
        println!("Temperatures: {}K at day, {}K at night", temp_day, temp_night);
    }

    if scheme.day.gamma[0].is_nan() {
        for g in scheme.day.gamma.iter_mut() { *g = DEFAULT_GAMMA }
    }
    if scheme.night.gamma[0].is_nan() {
        for g in scheme.night.gamma.iter_mut() { *g = DEFAULT_GAMMA }
    }


    if verbose {
        loc.print();
    }

    if args.flag_p {
        let elev = solar::elevation(systemtime_get_time(), &loc);
        let color_setting = scheme.interpolate_color_settings(elev);
        println!("Color temperature: {:?}K", color_setting.temp);
        println!("Brightness: {:?}", color_setting.brightness);
        RedshiftError::PrintMode.exit();
    }

    let mut gamma_state = gamma_randr::RandrMethod.init();
    gamma_state.start();



    /* Run continual mode */


    // Create signal thread
    let sigint = chan_signal::notify(&[chan_signal::Signal::INT,
                                       chan_signal::Signal::TERM]);
    let (signal_tx, signal_rx) = chan::sync(0);
    thread::spawn(move || {
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
    thread::spawn(move || {
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
                    if verbose {
                        period.print();
                    }
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

                if verbose {
                    if color_setting.temp != prev_color_setting.temp {
                        println!("Color temperature: {:?}K", color_setting.temp);
                    }
                    if color_setting.brightness != prev_color_setting.brightness {
                        println!("Brightness: {:?}", color_setting.brightness);
                    }
                }
                gamma_state.set_temperature(&color_setting);

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
