use xcb;
use xcb::randr;
use transition;
use colorramp;

use gamma_method::{GammaMethod, GammaMethodProvider};
use super::Result;
use std::error::Error;
use std::fmt;

/// Wrapper for XCB and RandR errors
pub enum RandrError<T> {
    Generic(xcb::Error<T>),
    Conn(xcb::ConnError)
}

impl<T: 'static> RandrError<T> {
    fn generic(e: xcb::Error<T>) -> Box<Error> {
        Box::new(RandrError::Generic(e)) as Box<Error>
    }
}

impl RandrError<()> {
    fn conn(e: xcb::ConnError) -> Box<Error> {
        Box::new(RandrError::Conn::<()>(e)) as Box<Error>
    }
}

impl<T> fmt::Display for RandrError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl<T> fmt::Debug for RandrError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::RandrError::*;
        match *self {
            Generic(ref e) =>
                write!(f, "randr error: {}", e.error_code()),
            Conn(xcb::ConnError::Connection) =>
                write!(f, "xcb connection errors because of socket, pipe or other stream errors"),
            Conn(ref c) =>
                write!(f, "{:?}", c),
        }
    }
}

impl<T> Error for RandrError<T> {
    fn description(&self) -> &str {
        "RandR error"
    }
}

const RANDR_MAJOR_VERSION: u32 = 1;
const RANDR_MINOR_VERSION: u32 = 3;

struct Crtc {
    id: u32,
    ramp_size: u32,
    saved_ramps: (Vec<u16>, Vec<u16>, Vec<u16>)
}

/**
 * Wrapping struct for RandR state
 */
pub struct RandrState {
    conn: xcb::Connection,
    screen_num: i32,
    window_dummy: u32,
    crtcs: Vec<Crtc>
}

impl RandrState {

    // TODO(tj): Remove all traces of unwrap()
    fn init() -> Result<RandrState> {
        let (conn, screen_num) = xcb::Connection::connect(None)
            .map_err(RandrError::conn)?;

        query_version(&conn);

        let window_dummy = {
            let setup = conn.get_setup();
            let screen = setup.roots().nth(screen_num as usize).unwrap();
            let window_dummy = conn.generate_id();

            xcb::create_window(&conn, 0, window_dummy, screen.root(), 0, 0, 1,
                               1, 0, 0, 0, &[]);
            conn.flush();
            window_dummy
        };

        Ok(RandrState {
            conn: conn,
            screen_num: screen_num,
            window_dummy: window_dummy,
            crtcs: vec![]
        })
    }

    // Set the temperature for the indicated CRTC
    fn set_crtc_temperature(&self, setting: &transition::ColorSetting, crtc: &Crtc) -> Result<()> {
        /* Copy saved ramps from CRTC */
        let mut r = crtc.saved_ramps.0.clone();
        let mut g = crtc.saved_ramps.1.clone();
        let mut b = crtc.saved_ramps.2.clone();

        /* Create new gamma ramps */
        colorramp::colorramp_fill(&mut r[..], &mut g[..], &mut b[..],
                                  setting,
                                  crtc.ramp_size as usize);

        // TODO Use a scratch-pad, and only call
        // set_crtc_gamma_checked() when ramp values change
        randr::set_crtc_gamma_checked(&self.conn,
                                      crtc.id,
                                      &r[..],
                                      &g[..],
                                      &b[..])
            .request_check()
            .map_err(RandrError::generic)
    }
}

fn query_version(conn: &xcb::Connection) {
    let reply = randr::query_version(conn,
                                     RANDR_MAJOR_VERSION,
                                     RANDR_MINOR_VERSION).get_reply().unwrap();
    println!("RandR {}.{}", reply.major_version(),
             reply.minor_version());
}

impl GammaMethod for RandrState {

    //
    // Restore saved gamma ramps
    //
    fn restore(&self) {
        for crtc in self.crtcs.iter() {
            randr::set_crtc_gamma_checked(&self.conn,
                                          crtc.id,
                                          &crtc.saved_ramps.0[..],
                                          &crtc.saved_ramps.1[..],
                                          &crtc.saved_ramps.2[..]);
        }
    }

    fn set_temperature(&self, setting: &transition::ColorSetting) -> Result<()> {
        for crtc in self.crtcs.iter() {
            self.set_crtc_temperature(setting, crtc)?;
        }
        Ok(())
    }

    /**
     * Find initial information on all the CRTCs
     */
    fn start(&mut self) {
        /* Get list of CRTCs for the screen */
        let screen_resources = randr::get_screen_resources(&self.conn,
                                                           self.window_dummy).get_reply().unwrap();
        self.crtcs = Vec::with_capacity(screen_resources.num_crtcs() as usize);

        /* Save size and gamma ramps of all CRTCs */
        for crtc in screen_resources.crtcs() {
            let gamma = randr::get_crtc_gamma(&self.conn, *crtc).get_reply().unwrap();
            let red = gamma.red().to_vec();
            let green = gamma.green().to_vec();
            let blue = gamma.blue().to_vec();

            self.crtcs.push(Crtc {
                id: *crtc,
                ramp_size: gamma.size() as u32,
                saved_ramps: (red, green, blue)
            });
        }
    }
}


pub struct RandrMethod;
impl GammaMethodProvider for RandrMethod {
    fn init(&self) -> Result<Box<GammaMethod>> {
        RandrState::init().map(|r| Box::new(r) as Box<GammaMethod>)
    }
}
