mod dx12;
mod event;
mod input;
mod painter;
mod win32;

pub use dx12::painter_dx12::PainterDX12;
pub use input::WindowInput;
pub use painter::Painter;
pub use win32::Win32;
