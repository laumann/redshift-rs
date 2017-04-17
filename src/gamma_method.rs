use transition;
use super::Result;

/**
 * Impl for any gamma adjustment method
 */
pub trait GammaMethod {
    fn restore(&self);

    fn set_temperature(&self, setting: &transition::ColorSetting) -> Result<()>;

    fn start(&mut self);
}

/// A gamma method provider serves can initialize a new gamma method.
///
/// TODO(tj): This is terribly Java-esque in my opinion, but I
///           currently do not have a better way to handle this.
pub trait GammaMethodProvider {
    fn init(&self) -> Result<Box<GammaMethod>>;
}

pub struct DummyMethod;
impl GammaMethod for DummyMethod {
    fn restore(&self) {}

    fn set_temperature(&self, setting: &transition::ColorSetting) -> Result<()> {
        println!("Temperature: {}", setting.temp);
        Ok(())
    }

    fn start(&mut self) {
        println!("WARNING: Using dummy gamma method! Display will not affected by this gamma method.");
    }
}

impl GammaMethodProvider for DummyMethod {
    fn init(&self) -> Result<Box<GammaMethod>> {
        Ok(Box::new(DummyMethod) as Box<GammaMethod>)
    }
}
