#[repr(C)]
#[derive(Debug)]
pub struct PortEvent {
    pub port: u16,
    pub reject: bool,
}
