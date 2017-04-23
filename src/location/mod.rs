/// Determining location
///
/// Module for different location providers. Can be manual or provided
/// by some service.

#[cfg(feature = "geoclue2")]
mod geoclue2;

use std::str::FromStr;
use super::{Result, RedshiftError};
use std::error::Error;

/**
 * Latitude and longitude location
 */
pub struct Location {
    pub lat: f64,
    pub lon: f64
}

impl Location {
    pub fn new(lat: f64, lon: f64) -> Location {
        Location {
            lat: lat,
            lon: lon
        }
    }

    pub fn print(&self) {
        // TODO(tj): Print N/E/S/W for lat/lon
        println!("Location {:2}, {:2}", self.lat, self.lon);
    }
}


impl FromStr for Location {
    type Err = Box<Error>;

    fn from_str(s: &str) -> Result<Location> {
        #[inline]
        fn m<T>(msg: String) -> Result<T> {
            Err(Box::new(RedshiftError::MalformedArgument(msg)))
        }

        let mut parts = s.split(':');

        let lat = parts.next()
            .map_or(m(format!("location: {}", s)),
                    |l| l.parse().or(m(format!("location: {} (of {})", l, s))))?;

        let lon = parts.next()
            .map_or(m(format!("location: {}", s)),
                    |l| l.parse::<f64>().or(m(format!("location: {} (of {})", l, s))))?;

        parts.next()
            .map_or(Ok(Location::new(lat, lon)),
                    |trailing| m(format!("location: trailing {} (of {})", trailing, s)))
    }
}

/// Determine the current location from the given argument.
///
/// The location can either be specified as <LAT:LON> or by naming a
/// particular location provider. If a provider exists whose name
/// matches the input, then that provider is tried. Otherwise, the
/// argument is attempted parsed as "LAT:LON".
///
/// If the location argument is omitted, a default is chosen.
pub fn determine(location_arg: Option<&str>) -> Result<Location> {
    match location_arg {
        Some(loc) => {
            // Look for provider and use if matched, otherwise parse
            // as LAT:LON.
            loc.parse::<Location>()
        }
        None => Ok(Location::new(55.7, 12.6))
    }
}
