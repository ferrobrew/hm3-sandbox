use windows::Win32::Foundation::{LPARAM, WPARAM};

pub const fn loword(value: u32) -> u16 {
    (value & 0xFFFF) as u16
}

pub const fn hiword(value: u32) -> u16 {
    ((value >> 16) & 0xFFFF) as u16
}

pub const fn xparam(value: u32) -> i16 {
    loword(value) as i16
}

pub const fn yparam(value: u32) -> i16 {
    hiword(value) as i16
}

pub trait Win32 {
    fn dword(&self) -> u32;

    fn loword(&self) -> u16 {
        loword(self.dword())
    }

    fn hiword(&self) -> u16 {
        hiword(self.dword())
    }

    fn xparam(&self) -> i16 {
        xparam(self.dword())
    }

    fn yparam(&self) -> i16 {
        yparam(self.dword())
    }
}

impl Win32 for LPARAM {
    fn dword(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }
}

impl Win32 for WPARAM {
    fn dword(&self) -> u32 {
        (self.0 & 0xFFFFFFFF) as u32
    }
}
