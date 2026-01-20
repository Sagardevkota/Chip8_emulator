use wasm_bindgen::prelude::*;

use crate::Chip8;

#[wasm_bindgen]
pub struct Chip8Wasm {
    inner: Chip8,
}

#[wasm_bindgen]
impl Chip8Wasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Chip8Wasm {
        Chip8Wasm { inner: Chip8::new() }
    }

    pub fn load_pong(&mut self) {
        self.inner.load_rom(include_bytes!("../roms/Pong.ch8"));
    }

    pub fn set_key(&mut self, key: u8, pressed: bool) {
        let idx = key as usize;
        if idx < self.inner.keypad.len() {
            self.inner.keypad[idx] = pressed;
        }
    }

    pub fn tick(&mut self) {
        for _ in 0..10 {
            let opcode = self.inner.fetch();
            self.inner.decode_execute(opcode);
        }
        self.inner.tick_timers();
    }

    pub fn frame(&self) -> Vec<u8> {
        self.inner.display.to_vec()
    }
}
