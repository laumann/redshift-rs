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

extern crate clap;
extern crate rustc_serialize;

use clap::{App, AppSettings, Arg, ArgSettings};

use std::thread;
use std::fmt;
use gamma_method::GammaMethodProvider;
use std::result;
use std::error::Error;

mod transition;
mod colorramp;
mod location;
mod solar;
mod gamma_method;
mod gamma_randr;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

const ABOUT: &'static str = "
Set color temperature of display according to time of day.

A Rust clone of the original Redshift written in C by Jon Lund Steffensen.";

const USAGE: &'static str = "\
    redshift-rs [OPTIONS]
    redshift-rs (-h | --help)
    redshift-rs (-V | --version)";

pub type Result<T> = result::Result<T, Box<Error + Send + Sync>>;

// Constants
const NEUTRAL_TEMP:        i32 = 6500;
const DEFAULT_DAY_TEMP:    i32 = 5500;
const DEFAULT_NIGHT_TEMP:  i32 = 3500;
const DEFAULT_BRIGHTNESS:  f64 = 1.0;
const DEFAULT_GAMMA:       f64 = 1.0;

// Error codes returned
// TODO(tj): Improve how this is presented
#[derive(Debug)]
pub enum RedshiftError {
    MalformedArgument(String),
}

impl fmt::Display for RedshiftError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for RedshiftError {
    fn description(&self) -> &str {
        "redshift error"
    }
}

fn app<'app>() -> App<'app, 'app> {
    let arg = |name| Arg::with_name(name).long(name);
    App::new("redshift-rs")
        .author("Thomas Jespersen <laumann.thomas@gmail.com>")
        .version(VERSION)
        .about(ABOUT)
        .usage(USAGE)
        .max_term_width(80)
        .setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::ColorNever)
        .arg(arg("brightness")
             .short("b")
             .value_name("DAY:NIGHT")
             .help("Screen brightness to apply (between 0.1 and 1.0)"))
        .arg(arg("temperature")
             .short("t")
             .value_name("DAY:NIGHT")
             .help("Set day/night color temperatures"))
        .arg(arg("no-transition").short("r").help("Disable temperature transitions"))
        .arg(arg("print").short("p")
             .help("Print parameters and exit")
             .conflicts_with_all(&["oneshot", "reset", "oneshot-manual"]))
        .arg(arg("oneshot").short("o")
             .help("One shot mode")
             .conflicts_with_all(&["print", "reset", "oneshot-manual"]))
        .arg(arg("oneshot-manual").short("O")
             .help("One shot mode (set color temperature)")
             .value_name("TEMP")
             .conflicts_with_all(&["print", "oneshot", "reset"]))
        .arg(arg("reset").short("x").help("Reset (remove adjustments to screen)"))
        .arg(arg("verbose").short("v").help("Verbose output"))
}

/// Selected run mode
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
#[allow(dead_code)]
enum Mode {
    /// Run the color adjustment method once and exit
    OneShot,

    /// Continually run the color adjustment
    ///
    /// This is the default mode
    Continual,

    /// Reset the screen
    Reset,

    /// Print parameters only and exit
    Print,

    /// One shot manual mode - set color temperature
    Manual(i32)
}

struct Args {
    pub verbose: bool,
    pub brightness: (f64, f64),
    pub location: location::Location,
    pub method: Option<String>,
    pub temperatures: (i32, i32),
    pub disable_transition: bool,
    pub mode: Mode,
}

impl Args {

    /// Parse the command-line arguments into a Redshift configuration
    pub fn parse() -> Result<Args> {
        let matches = app().get_matches();

        let brightness = matches.value_of("brightness")
            .map_or(Ok((DEFAULT_BRIGHTNESS, DEFAULT_BRIGHTNESS)),
                    |input| parse_brightness(input))?;

        let temperatures = matches.value_of("temperature")
            .map_or(Ok((DEFAULT_DAY_TEMP, DEFAULT_NIGHT_TEMP)),
                    |input| parse_temperature(input))?;

        // Determine run mode
        let mode = if matches.is_present("print") {
            Mode::Print
        } else if matches.is_present("oneshot") {
            Mode::OneShot
        } else if let Some(temp) = matches.value_of("oneshot-manual") {
            Mode::Manual(temp.parse()?)
        } else if matches.is_present("reset") {
            Mode::Reset
        } else {
            Mode::Continual
        };

        Ok(Args {
            verbose: matches.is_present("verbose"),
            brightness: brightness,
            location: location::Location::new(55.7, 12.6),
            method: None,
            temperatures: temperatures,
            disable_transition: matches.is_present("no-transition"),
            mode: mode,
        })
    }
}

#[inline]
fn malformed<T>(msg: String) -> Result<T> {
    Err(Box::new(RedshiftError::MalformedArgument(msg)))
}

/// Parse the temperature argument
///
/// Expected as "DAY:NIGHT", where DAY and NIGHT are floating point
/// numbers. Any other input produces an error
fn parse_temperature(input: &str) -> Result<(i32, i32)> {
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

fn parse_brightness(input: &str) -> Result<(f64, f64)> {
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
    match Args::parse().and_then(run) {
        Ok(exit_code) => ::std::process::exit(exit_code),
        Err(e) => {
            println!("{}", e);
            ::std::process::exit(1);
        }
    }
}

// (3) Running continual mode (if requested)
fn run(args: Args) -> Result<i32> {

    let verbose = args.verbose;

    // Init location
    let loc = args.location;
    let (temp_day, temp_night) = args.temperatures;
    let (bright_day, bright_night) = args.brightness;

    // Init transition scheme
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

    if args.mode == Mode::Print {
        // TODO(tj): Print period as well
        let elev = solar::elevation(systemtime_get_time(), &loc);
        let color_setting = scheme.interpolate_color_settings(elev);
        println!("Color temperature: {:?}K", color_setting.temp);
        println!("Brightness: {:.2}", color_setting.brightness);
        return Ok(0)
    }

    let mut gamma_state = gamma_randr::RandrMethod.init();
    gamma_state.start();



    // Running continual mode
    // TODO(tj): Match on args.mode to determine run mode; move the
    // inner body of the loop into its own function and move all
    // looping and channel handling to a run_continual() function

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
    Ok(0)
}

fn systemtime_get_time() -> f64 {
    let now = time::get_time();
    now.sec as f64 + (now.nsec as f64 / 1_000_000_000.0)
}
