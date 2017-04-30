//!
//! # Redshift in Rust
//!
//! aka redshift-rs
//! aka rustshift
//!

extern crate time;
#[macro_use]
extern crate chan;
extern crate chan_signal;
#[macro_use] extern crate lazy_static;
extern crate ini;

extern crate clap;

// Optional features for gamma method providers
#[cfg(feature = "randr")] extern crate xcb;

// Optional features for location providers
#[cfg(feature = "geoclue2")] extern crate dbus;

use std::thread;
use std::fmt;
use std::result;
use std::error::Error;

use clap::{App, AppSettings, Arg};

mod transition;
mod colorramp;
mod location;
mod solar;
mod gamma;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

const ABOUT: &'static str = "
Set color temperature of display according to time of day.

A Rust clone of the original Redshift written in C by Jon Lund Steffensen.";

const USAGE: &'static str = "\
    redshift-rs [OPTIONS]
    redshift-rs (-h | --help)
    redshift-rs (-V | --version)";

pub type Result<T> = result::Result<T, Box<Error>>;

// Constants
const NEUTRAL_TEMP:        i32 = 6500;
const DEFAULT_DAY_TEMP:    i32 = 5500;
const DEFAULT_NIGHT_TEMP:  i32 = 3500;
const MIN_TEMP:            i32 = 1000;
const MAX_TEMP:            i32 = 25000;
const DEFAULT_BRIGHTNESS:  f64 = 1.0;
const DEFAULT_GAMMA:       f64 = 1.0;
const MIN_GAMMA:           f64 = 0.1;
const MAX_GAMMA:           f64 = 10.0;


// Error codes returned
#[derive(Debug)]
pub enum RedshiftError {
    MalformedArgument(String),
    MalformedConfig(String),
    GammaMethodNotFound(String),
}

impl fmt::Display for RedshiftError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use RedshiftError::*;
        match *self {
            MalformedArgument(ref msg) =>
                write!(f, "malformed argument: {}", msg),
            MalformedConfig(ref msg) =>
                write!(f, "malformed configuration: {}", msg),
            GammaMethodNotFound(ref method_name) =>
                write!(f, "gamma method '{}' not found", method_name),
        }
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
        .setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::ColorNever)
        .arg(arg("brightness")
             .short("b")
             .value_name("DAY:NIGHT")
             .help("Screen brightness to apply (between 0.1 and 1.0)"))
        .arg(arg("method")
             .short("m")
             .value_name("METHOD")
             .help("Method to use to set color temperature"))
        .arg(arg("location")
             .short("l")
             .value_name("LAT:LON")
             .help("Your current location"))
        .arg(arg("temperature")
             .short("t")
             .value_name("DAY:NIGHT")
             .help("Set day/night color temperatures"))
        .arg(arg("gamma")
             .short("g")
             .value_name("R:G:B")
             .help("Additional gamma correction to apply"))
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
    pub gamma: (f64, f64, f64),
    pub location: location::Location,
    pub method: Option<String>,
    pub temperatures: (i32, i32),
    pub transition: bool,
    pub mode: Mode,
}

impl Args {

    pub fn defaults() -> Args {
        Args {
            verbose: false,
            brightness: (DEFAULT_BRIGHTNESS, DEFAULT_BRIGHTNESS),
            gamma: (DEFAULT_GAMMA, DEFAULT_GAMMA, DEFAULT_GAMMA),
            location: location::Location::new(55.7, 12.6),
            method: None,
            temperatures: (DEFAULT_DAY_TEMP, DEFAULT_NIGHT_TEMP),
            transition: true,
            mode: Mode::Continual,
        }
    }

    pub fn update_from_config(mut self) -> Result<Args> {
        let conf = std::env::home_dir()
            .map(|mut path| { path.push(".config/redshift.conf"); path })
            .and_then(|home| ini::Ini::load_from_file(&home).ok());

        let conf = if let Some(c) = conf { c } else { return Ok(self) };

        let section = conf.section(Some("redshift")).map(Ok)
            .unwrap_or(malformed_config(format!("config file does not have a 'redshift' section")))?;

        if let Some(brightness_day) = section.get("brightness-day") {
            self.brightness.0 = brightness_day.parse()
                .or_else(|e| malformed_config(format!("could not parse brightness-day: {}", e)))?;
        }
        if let Some(brightness_night) = section.get("brightness-night") {
            self.brightness.1 = brightness_night.parse()
                .or_else(|e| malformed_config(format!("could not parse brightness-night: {}", e)))?;
        }

        if let Some(temp_day) = section.get("temp-day") {
            self.temperatures.0 = temp_day.parse()
                .or_else(|e| malformed_config(format!("could not parse temp-day: {}", e)))?;
        }
        if let Some(temp_night) = section.get("temp-night") {
            self.temperatures.1 = temp_night.parse()
                .or_else(|e| malformed_config(format!("could not parse temp-night: {}", e)))?;
        }

        if let Some(gamma) = section.get("gamma") {
            self.gamma = parse_gamma(gamma)?;
        }

        if let Some(transition) = section.get("transition") {
            self.transition = transition != "0";
        }

        if let Some("manual") = section.get("location-provider").map(|s| s.as_str()) {
            let lat = conf.get_from(Some("manual"), "lat");
            let lon = conf.get_from(Some("manual"), "lon");
            match (lat, lon) {
                (Some(lat), Some(lon)) => {
                    let lat = lat.parse()
                        .or_else(|e| malformed_config(format!("could not parse latitude: {}", e)))?;
                    let lon = lon.parse()
                        .or_else(|e| malformed_config(format!("could not parse longitude: {}", e)))?;
                    self.location = location::Location::new(lat, lon);
                }
                _ => {
                    return malformed_config(format!("missing 'lat' or 'lon' value for 'manual' location provider"));
                }
            }
        }

        if let Some(method) = section.get("adjustment-method") {
            self.method = determine_gamma_method(method.to_owned())
                .or_else(|e| malformed_config(format!("{}", e)))
                .map(Some)?;
        }

        Ok(self)
    }

    /// Parse the command-line arguments into a Redshift configuration
    pub fn update_from_args(mut self) -> Result<Args> {
        let matches = app().get_matches();

        if let Some(input) = matches.value_of("brightness") {
            self.brightness = parse_brightness(input)?;
        }

        if let Some(input) = matches.value_of("temperature") {
            self.temperatures = parse_temperature(input)?;
        }

        if let Some(input) = matches.value_of("gamma") {
            self.gamma = parse_gamma(input)?;
        }

        // Determine run mode
        self.mode = if matches.is_present("print") {
            Mode::Print
        } else if matches.is_present("oneshot") {
            Mode::OneShot
        } else if let Some(temp) = matches.value_of("oneshot-manual") {
            let t = temp.parse()?;
            if t < MIN_TEMP || t > MAX_TEMP {
                return malformed(format!("Temperature must be between {} and {} (was {})", MIN_TEMP, MAX_TEMP, t));
            }
            Mode::Manual(t)
        } else if matches.is_present("reset") {
            Mode::Reset
        } else {
            self.mode
        };

        if let Some(location) = matches.value_of("location") {
            self.location = location.parse()?;
        }

        if let Some(method) = matches.value_of("method") {
            self.method = determine_gamma_method(method.to_owned()).map(Some)?;
        }

        self.verbose = matches.is_present("verbose");
        self.transition = !matches.is_present("no-transition");

        Ok(self)
    }
}

#[inline]
fn malformed<T>(msg: String) -> Result<T> {
    Err(Box::new(RedshiftError::MalformedArgument(msg)))
}

#[inline]
fn malformed_config<T>(msg: String) -> Result<T> {
    Err(Box::new(RedshiftError::MalformedConfig(msg)))
}

fn determine_gamma_method(method: String) -> Result<String> {
    if gamma::is_method_available(&method[..]) {
        Ok(method)
    } else {
        Err(Box::new(RedshiftError::GammaMethodNotFound(method)))
    }
}

/// Parse the temperature argument
///
/// Expected as "DAY:NIGHT", where DAY and NIGHT are 32-bit
/// integers. Any other input produces an error.
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

/// Parse brightness argument
///
/// Expected format is "DAY:NIGHT" where DAY and NIGHT are floating
/// point numbers. Any other input produces an error.
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


/// A gamma string contains either one floating point value, or three
/// separated by colons
fn parse_gamma(input: &str) -> Result<(f64, f64, f64)> {
    macro_rules! validate_gamma {
        ($val:ident) => (
            if $val < MIN_GAMMA || $val > MAX_GAMMA {
                return malformed(format!("Gamma value must be between {} and {}. Was {}",
                                         MIN_GAMMA, MAX_GAMMA, $val));
            }
        )
    }


    let mut parts = input.split(':');

    let fst = parts.next()
        .map_or(malformed(format!("gamma: {}", input)),
                |l| l.parse().or(
                    malformed(format!("gamma: {} (of {})", l,
                                      input))))?;
    validate_gamma!(fst);

    if let Some(l) = parts.next() {
        let g = l.parse().or(malformed(format!("gamma: {} (of {})", l,
                                               input)))?;
        validate_gamma!(g);

        let b = parts.next()
            .map_or(malformed(format!("gamma: {} (of {})", l, input)),
                    |l| l.parse().or(
                        malformed(format!("gamma: {} (of {})", l,
                                          input))))?;
        validate_gamma!(b);
        Ok((fst, g, b))
    } else {
        Ok((fst, fst, fst))
    }
}

fn main() {
    let result = Args::defaults().update_from_config()
        .and_then(|args| args.update_from_args())
        .and_then(run);
    ::std::process::exit(match result {
        Ok(exit_code) => {
            exit_code
        }
        Err(e) => {
            println!("{}", e);
            1
        }
    });
}

fn run(args: Args) -> Result<i32> {
    let (temp_day, temp_night) = args.temperatures;
    let (bright_day, bright_night) = args.brightness;

    // Init transition scheme
    let mut scheme = transition::TransitionScheme::new();
    scheme.day.temp = temp_day;
    scheme.night.temp = temp_night;
    scheme.day.brightness = bright_day;
    scheme.night.brightness = bright_night;

    scheme.day.gamma[0] = args.gamma.0;
    scheme.day.gamma[1] = args.gamma.1;
    scheme.day.gamma[2] = args.gamma.2;

    scheme.night.gamma[0] = args.gamma.0;
    scheme.night.gamma[1] = args.gamma.1;
    scheme.night.gamma[2] = args.gamma.2;

    if args.verbose {
        println!("Temperatures: {}K at day, {}K at night", temp_day, temp_night);
        println!("{}", args.location);
    }

    match args.mode {
        Mode::Reset => {
            let mut gamma_state = gamma::init_gamma_method(args.method.as_ref().map(|s| s.as_str()))?;
            gamma_state.start()?;
            gamma_state.set_temperature(&transition::ColorSetting {
                temp: NEUTRAL_TEMP,
                gamma: [1.0, 1.0, 1.0],
                brightness: 1.0
            })?;
        }
        Mode::OneShot | Mode::Print => {
            let now = systemtime_get_time();
            let print = args.verbose || args.mode == Mode::Print;

            // Compute elevation
            let elev = solar::elevation(now, &args.location);

            let period = scheme.get_period(elev);

            // Interpolate between 6500K and calculated temperature
            let color_setting = scheme.interpolate_color_settings(elev);

            if print {
                println!("Solar elevation: {}", elev);
                period.print();
                println!("Color temperature: {}K", color_setting.temp);
                println!("Brightness: {:.2}", color_setting.brightness);
            }

            if args.mode == Mode::OneShot {
                let mut gamma_state = gamma::init_gamma_method(args.method.as_ref().map(|s| s.as_str()))?;
                gamma_state.start()?;
                gamma_state.set_temperature(&color_setting)?;
            }
        }
        Mode::Manual(temp) => {
            if args.verbose {
                println!("Color temperature: {}", temp);
            }
            let color_setting = transition::ColorSetting {
                temp: temp,
                gamma: scheme.day.gamma.clone(),
                brightness: scheme.day.brightness
            };

            let mut gamma_state = gamma::init_gamma_method(args.method.as_ref().map(|s| s.as_str()))?;
            gamma_state.start()?;
            gamma_state.set_temperature(&color_setting)?;
        }
        Mode::Continual => {
            run_continual_mode(args, scheme)?;
        }
    }
    Ok(0)
}

/// Continual mode
///
/// The default functionality of Redshift is to run continually
/// adjusting the temperature as the day progresses. It is interrupted
/// by signals INT and TERM that both cause it to terminate.
///
/// TODO: Respect the transition scheme, espectially in the presence
///       of the --no-transition flag
fn run_continual_mode(args: Args, mut scheme: transition::TransitionScheme) -> Result<()> {
    let mut gamma_state = gamma::init_gamma_method(args.method.as_ref().map(|s| s.as_str()))?;
    gamma_state.start()?;

    // Create signal thread
    let sigint = chan_signal::notify(&[chan_signal::Signal::INT,
                                       chan_signal::Signal::TERM]);
    let (signal_tx, signal_rx) = chan::sync(0);
    thread::spawn(move || {
        for sig in sigint.iter() {
            signal_tx.send(sig);
        }
    });

    let (timer_tx, timer_rx) = chan::sync(0);
    let (sleep_tx, sleep_rx) = chan::sync(0);
    thread::spawn(move || {
        for ms in sleep_rx.iter() {
            thread::sleep(std::time::Duration::from_millis(ms));
            timer_tx.send(());
        }
    });

    let mut now;
    let mut exiting = false;
    let mut prev_color_setting = transition::ColorSetting::new();
    let mut prev_period = transition::Period::None;
    sleep_tx.send(0);
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
                let elev = solar::elevation(now, &args.location);

                let period = scheme.get_period(elev);
                if period != prev_period {
                    if args.verbose {
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

                if args.verbose {
                    if color_setting.temp != prev_color_setting.temp {
                        println!("Color temperature: {:?}K", color_setting.temp);
                    }
                    if color_setting.brightness != prev_color_setting.brightness {
                        println!("Brightness: {:?}", color_setting.brightness);
                    }
                }
                if color_setting != prev_color_setting {
                    gamma_state.set_temperature(&color_setting)?;
                }

                if exiting && !scheme.short_transition() {
                    break
                }

                // Sleep for 5 seconds or 0.1 second
                sleep_tx.send(if scheme.short_transition() { 100 } else { 5000 });

                /* Save temperature */
                prev_color_setting = color_setting;
            }
        }
    }
    gamma_state.restore()?;
    Ok(())
}

fn systemtime_get_time() -> f64 {
    let now = time::get_time();
    now.sec as f64 + (now.nsec as f64 / 1_000_000_000.0)
}
