use std::str::FromStr;
use super::RedshiftError;

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
        println!("Location {:2}, {:2}", self.lat, self.lon);
    }
}

impl FromStr for Location {
    type Err = RedshiftError;

    fn from_str(s: &str) -> Result<Location, Self::Err> {
        #[inline] fn m<T>(msg: String) -> Result<T, RedshiftError> {
            Err(RedshiftError::MalformedArgument(msg))
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
