/**
 * Compute solar zenith angle/solar elevation angle
 *
 * Adapted from the Redshift source code (which in turn was adapted
 * from some JavaScript code)
 */
use time;
use location;

/**
 * Model of atmospheric refraction near horizon (in degrees)
 */
#[test] pub const SOLAR_ATM_REFRAC: f64 = 0.833;

/**
 * Various elevation constants
 */
#[test] pub const ASTRO_TWILIGHT_ELEV: f64 = -18.0;
#[test] pub const NAUT_TWILIGHT_ELEV:  f64 = -12.0;
        pub const CIVIL_TWILIGHT_ELEV: f64 = -6.0;
#[test] pub const DAYTIME_ELEV:        f64 = (0.0 - SOLAR_ATM_REFRAC);

/**
 * Solar times - see the time_angle[] array
 */

#[test] pub const NOON:       usize = 0;
#[test] pub const MIDNIGHT:   usize = 1;
#[test] pub const ASTRO_DAWN: usize = 2;
#[test] pub const NAUT_DAWN:  usize = 3;
#[test] pub const CIVIL_DAWN: usize = 4;
#[test] pub const SUNRISE:    usize = 5;
#[test] pub const SUNSET:     usize = 6;
#[test] pub const CIVIL_DUSK: usize = 7;
#[test] pub const NAUT_DUSK:  usize = 8;
#[test] pub const ASTRO_DUSK: usize = 9;

/**
 * Computed angles (or angels), these can be re-computed using the
 * time_angles() test function (see below)
 */
// pub const time_angle: [f64; 10] = [
//     0.0,                 // NOON (not used)
//     ::std::f64::NAN,     // MIDNIGHT (not used)

//     -1.8849555921538759, // ASTRO_DAWN
//     -1.7802358370342162, // NAUT_DAWN
//     -1.6755160819145565, // CIVIL_DAWN
//     -1.5853349194640094, // SUNRISE
//     1.5853349194640094,  // SUNSET
//     1.6755160819145565,  // CIVIL_DUSK
//     1.7802358370342162,  // NAUT_DUSK
//     1.8849555921538759,  // ASTRO_DUSK
// ];

/* A Julian Day */
pub type JulianDay = f64;
pub trait JulianDays {
    fn from_epoch(t: f64) -> JulianDay;
    fn to_julian_cent(self) -> JulianCent;
    fn to_epoch(self) -> f64;
}
impl JulianDays for JulianDay {
    fn from_epoch(t: f64) -> JulianDay {
        ((t / 86400.0) + 2440587.5) as JulianDay
    }

    fn to_epoch(self) -> f64 {
        (self - 2440587.5) * 86400.0
    }

    fn to_julian_cent(self) -> JulianCent {
        ((self - 2451545.0) / 36525.0) as JulianCent
    }
}

/* A Julian century since J2000.0  */
pub type JulianCent = f64;
pub trait JulianCents {
    fn to_julian_day(self) -> JulianDay;
    fn sun_geom_mean_lon(self) -> f64;
    fn sun_geom_mean_anomaly(self) -> f64;
    fn earth_orbit_eccentricity(self) -> f64;
    fn sun_equation_of_center(self) -> f64;
    fn sun_true_lon(self) -> f64;
    fn sun_apparent_lon(self) -> f64;
    fn mean_ecliptic_obliquity(self) -> f64;
    fn obliquity_corr(self) -> f64;
    fn solar_declination(self) -> f64;
    fn equation_of_time(self) -> f64;
}
impl JulianCents for JulianCent {
    fn to_julian_day(self) -> JulianDay {
        ((self * 36525.0) + 2451545.0) as JulianDay
    }

    fn sun_geom_mean_lon(self) -> f64 {
        ((280.46646 + self * (36000.76983 + self * 0.0003032)) % 360.0).to_radians()
    }

    fn sun_geom_mean_anomaly(self) -> f64 {
        (357.52911 + self * (35999.05029 - self * 0.0001537)).to_radians()
    }

    fn earth_orbit_eccentricity(self) -> f64 {
        0.016708634 - self * (0.000042037 + self * 0.0000001267)
    }

    fn sun_equation_of_center(self) -> f64 {
        let m = self.sun_geom_mean_anomaly();
        let c = m.sin() * (1.914602 - self * (0.004817 + 0.000014 * self))
            + (2.0*m).sin() * (0.019993 - 0.000101 * self)
            + (3.0*m).sin() * 0.000289;
        c.to_radians()
    }

    fn sun_true_lon(self) -> f64 {
        self.sun_geom_mean_lon() + self.sun_equation_of_center()
    }

    /* Apparent longitude of the sun (right ascension) */
    fn sun_apparent_lon(self) -> f64 {
        let o = self.sun_true_lon();
        (o.to_degrees() - 0.00569 - 0.00478 * (125.04 - 1934.136 * self).to_radians().sin()).to_radians()
    }

    fn mean_ecliptic_obliquity(self) -> f64 {
        let sec = 21.448 - self * (46.815 + self * (0.00059 - self * 0.001813));
        (23.0 + (26.0 + (sec/60.0))/60.0).to_radians()
    }

    fn obliquity_corr(self) -> f64 {
        let e0 = self.mean_ecliptic_obliquity();
        let omega = 125.04 - self * 1934.136;
        (e0.to_degrees() + 0.00256 * omega.to_radians().cos()).to_radians()
    }

    fn solar_declination(self) -> f64 {
        let e = self.obliquity_corr();
        let lambda = self.sun_apparent_lon();
        (e.sin() * lambda.sin()).asin()
    }

    /* Difference between true solar time and mean solar time */
    fn equation_of_time(self) -> f64 {
        let l0 = self.sun_geom_mean_lon();
        let e = self.earth_orbit_eccentricity();
        let m = self.sun_geom_mean_anomaly();
        let y = (self.obliquity_corr()/2.0).tan().powf(2.0);

        let eq_time = y * (2.0 * l0).sin()
            - 2.0 * e * m.sin()
            + 4.0 * e * y * m.sin() * (2.0* l0).cos()
            - 0.5 * y * y * (4.0 * l0).sin()
            - 1.25 * e * e * (2.0 * m).sin();
        4.0 * eq_time.to_degrees()
    }
}

// #[inline]
// fn copysign(x: f64, y: f64) -> f64 {
//     if y.is_sign_positive() ^ x.is_sign_positive() { -x } else { x }
// }

// pub fn hour_angle_from_elevation(lat: f64, decl: f64, elev: f64) -> f64 {
//     let omega = (elev.abs().cos() - lat.to_radians().sin() * decl.sin()).acos()
//         / (lat.to_radians().cos() * decl.cos());
//     copysign(omega, -elev)
// }

pub fn elevation_from_hour_angle(lat: f64, decl: f64, ha: f64) -> f64 {
    (ha.cos() * lat.to_radians().cos() * decl.cos()
     + lat.to_radians().sin() * decl.sin()).asin()
}

pub fn elevation_from_time(jd: JulianDay, loc: &location::Location) -> f64 {
    let t = jd.to_julian_cent();
    let offset = (jd - jd.round() - 0.5) * 1440.0;

    let eq_time = t.equation_of_time();
    let ha = ((720.0 - offset - eq_time)/4.0 - loc.lon).to_radians();
    let decl = t.solar_declination();
    elevation_from_hour_angle(loc.lat, decl, ha)
}

/* Compute the solar angular elevation at the given location and time */
pub fn elevation(t: f64, loc: &location::Location) -> f64 {
    let jd = JulianDay::from_epoch(t);
    elevation_from_time(jd, loc).to_degrees()
}

#[cfg(test)]
mod test {
    use super::*;
    use time;

    #[test]
    fn time_angles() {
        let angles = vec![(-90.0 + ASTRO_TWILIGHT_ELEV).to_radians(), // ASTRO_DAWN = 2
                          (-90.0 + NAUT_TWILIGHT_ELEV).to_radians(), // NAUT_DAWN = 3
                          (-90.0 + CIVIL_TWILIGHT_ELEV).to_radians(), // CIVIL_DAWN = 4
                          (-90.0 + DAYTIME_ELEV).to_radians(), // SUNRISE = 5
                          (0.0f64).to_radians(),                        // NOON = 0
                          (90.0 - DAYTIME_ELEV).to_radians(), // SUNSET = 6
                          (90.0 - CIVIL_TWILIGHT_ELEV).to_radians(), // CIVIL_DUSK = 7
                          (90.0 - NAUT_TWILIGHT_ELEV).to_radians(), // NAUT_DUSK = 8
                          (90.0 - ASTRO_TWILIGHT_ELEV).to_radians() // ASTRO_DUSK = 9
        ];
        for angle in angles {
            println!("{:?},", angle);
        }
    }

    #[test]
    fn zero_zero() {
        let jd1k = JulianDay::from_epoch(1000.0);
        let jd2k = JulianDay::from_epoch(10000.0);
        println!("eq_time(1000)={:?}", jd1k.to_julian_cent().equation_of_time());
        println!("eq_time(10000)={:?}", jd2k.to_julian_cent().equation_of_time());
        solar_elevation(1000, 0.0, 0.0);
    }
}
