use xcb;
use xcb::randr;
use transition;
use colorramp;

use gamma_method::{GammaMethod, GammaMethodProvider};

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

    fn init() -> RandrState {
        let (conn, screen_num) = xcb::Connection::connect(None).unwrap();

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

        RandrState {
            conn: conn,
            screen_num: screen_num,
            window_dummy: window_dummy,
            crtcs: vec![]
        }
    }

    fn set_crtc_temperature(&self, setting: &transition::ColorSetting, crtc: &Crtc) {
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
                                      &b[..]);
    }
}

fn query_version(conn: &xcb::Connection) {
    let reply = randr::query_version(conn,
                                     RANDR_MAJOR_VERSION,
                                     RANDR_MINOR_VERSION).get_reply().unwrap();
    println!("RandR {}.{}", reply.major_version(),
             reply.minor_version());
}

/**
 *
 */
impl GammaMethod for RandrState {

    /**
     * Restore saved gamma ramps
     */
    fn restore(&self) {
        for crtc in self.crtcs.iter() {
            randr::set_crtc_gamma_checked(&self.conn,
                                          crtc.id,
                                          &crtc.saved_ramps.0[..],
                                          &crtc.saved_ramps.1[..],
                                          &crtc.saved_ramps.2[..]);
        }
    }

    fn set_temperature(&self, setting: &transition::ColorSetting) {
        for crtc in self.crtcs.iter() {
            self.set_crtc_temperature(setting, crtc);
        }
    }

    /**
     * Find initial information on all the CRTCs
     */
    fn start(&mut self) {
        //let setup = self.conn.get_setup();

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
    fn init(&self) -> Box<GammaMethod> {
        Box::new(RandrState::init()) as Box<GammaMethod>
    }
}
