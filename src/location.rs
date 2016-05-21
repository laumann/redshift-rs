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
}
