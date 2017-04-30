/// Determining location
///
/// Module for different location providers. Can be manual or provided
/// by some service.

#[cfg(feature = "geoclue2")]
mod geoclue2;

use std::str::FromStr;
use super::{Result, RedshiftError};
use std::error::Error;
use std::fmt;

/// Location by latitude and longitude
pub struct Location {
    pub lat: f64,
    pub lon: f64
}

impl fmt::Display for Location {
    // TODO(tj): Print N/E/S/W for lat/lon
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Location {:2}, {:2}", self.lat, self.lon)
    }
}

impl Location {
    pub fn new(lat: f64, lon: f64) -> Location {
        Location {
            lat: lat,
            lon: lon
        }
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
