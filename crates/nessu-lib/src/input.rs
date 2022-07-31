#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

#[repr(u8)]
#[derive(Copy, Clone)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub enum Button {
    Down = 0b0000_0100,
    Up = 0b0000_1000,
    Right = 0b0000_0001,
    Left = 0b0000_0010,
    Start = 0b0001_0000,
    Select = 0b0010_0000,
    A = 0b1000_0000,
    B = 0b0100_0000,
}
