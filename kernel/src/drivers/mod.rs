pub mod keyboard;
pub mod mouse;
pub mod net;
pub mod pci;
pub mod storage;
pub mod timer;

pub fn init() {
    timer::init();
    keyboard::init();
    mouse::init(1280, 800);
    pci::init();
    storage::init();
    net::init();
}
