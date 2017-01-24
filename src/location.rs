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
        let mut parts = s.split(':');

        let lat = parts.next()
            .and_then(|l| l.parse::<f64>().ok())
            .ok_or(RedshiftError::MalformedArgument)?;

        let lon = parts.next()
            .and_then(|l| l.parse::<f64>().ok())
            .ok_or(RedshiftError::MalformedArgument)?;

        match parts.next() {
            Some(..) => Err(RedshiftError::MalformedArgument),
            None => Ok(Location::new(lat, lon))
        }
    }
}
