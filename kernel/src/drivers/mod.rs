pub mod keyboard;
pub mod pci;
pub mod timer;

pub fn init() {
    timer::init();
    keyboard::init();
    pci::init();
}
