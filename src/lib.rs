#[cfg(not(target_arch = "wasm32"))]
use minifb::{Key, Window};
use rand::Rng;
#[cfg(not(target_arch = "wasm32"))]
use rodio::Sink;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

const FONT_SET: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0  like ASCII those bits are high
    /* ****
     *  *
     *  *
     *  *
     ****
     */
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

//not from 0 as convention historical reasons
const FONT_START_ADDR: usize = 0x050;
pub struct Chip8 {
    //first 0x000 to 0x1FF is reserved
    pub ram: [u8; 4096], // 2n = 4096 means 12 bits required to address a location(we take max)
    pub pc: u16,         // we have to take u16 to accommodate 12 bits
    i: u16,              //index register not instruction register it's for drawing sprites
    pub vx: [u8; 16],    // v0..vE is general purpose vF is for flag
    pub display: [u8; 64 * 32],
    pub draw_flag: bool,
    pub stack: [u16; 16], //store return address and can only be 16 deep
    pub sp: u16,          // index to current entry in stack
    pub keypad: [bool; 16], //buffer that holds keys for specific key binds which is for moving
    delay_timer: u8,
    sound_timer: u8,
}

impl Chip8 {
    pub fn tick_timers(&mut self) {
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }

        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
    }
}

impl Chip8 {
    pub fn new() -> Self {
        let mut ram = [0u8; 4096];
        ram[FONT_START_ADDR..(FONT_START_ADDR + FONT_SET.len())].copy_from_slice(&FONT_SET);
        Self {
            ram,
            pc: 0,
            i: 0,
            vx: [0; 16],
            display: [0; 64 * 32],
            draw_flag: false,
            stack: [0; 16],
            sp: 0,
            keypad: [false; 16],
            delay_timer: 0,
            sound_timer: 0,
        }
    }

    pub fn load_rom(&mut self, data: &[u8]) {
        let start_addr: usize = 0x200;
        self.pc = start_addr as u16;
        let max_len = self.ram.len() - start_addr;
        let copy_len = data.len().min(max_len);
        let end_addr = start_addr + copy_len;
        self.ram[start_addr..end_addr].copy_from_slice(&data[..copy_len]);
    }

    pub fn fetch(&mut self) -> u16 {
        let high_byte = self.ram[self.pc as usize];
        let low_byte = self.ram[self.pc as usize + 1];
        self.pc += 2;
        //shift high bytes to left by 8 pos so first cast to 16
        let opcode = ((high_byte as u16) << 8) | (low_byte as u16);
        opcode
    }

    pub fn decode_execute(&mut self, opcode: u16) {
        let primary = (opcode & 0xF000) >> 12; // 0x0FFF is mask to just extract pos 12-15
        let x = ((opcode & 0x0F00) >> 8) as usize;
        let y = ((opcode & 0x00F0) >> 4) as usize;
        let n = (opcode & 0x000F) as u8;

        // Also common helpers:
        let nn = (opcode & 0x00FF) as u8;
        let nnn = opcode & 0x0FFF;

        let nibbles = (primary, x, y, n);
        match nibbles {
            // --- 0 Series ---
            (0x0, 0x0, 0xE, 0x0) => self.op_00e0(), //CLS
            (0x0, 0x0, 0xE, 0xE) => self.op_00ee(), // RET
            (0x0, _, _, _) => self.op_0nnn(nnn),    // SYS addr (Usually ignored)

            // --- Standard Logic/Flow ---
            (0x1, _, _, _) => self.op_1nnn(nnn),    // JP addr
            (0x2, _, _, _) => self.op_2nnn(nnn),    // CALL addr
            (0x3, _, _, _) => self.op_3xnn(x, nn),  // SE Vx, byte
            (0x4, _, _, _) => self.op_4xnn(x, nn),  // SNE Vx, byte
            (0x5, _, _, 0x0) => self.op_5xy0(x, y), // SE Vx, Vy
            (0x6, _, _, _) => self.op_6xnn(x, nn),  // LD Vx, byte
            (0x7, _, _, _) => self.op_7xnn(x, nn),  // ADD Vx, byte

            // --- 8 Series (Arithmetic) ---
            (0x8, _, _, 0x0) => self.op_8xy0(x, y), // LD Vx, Vy
            (0x8, _, _, 0x1) => self.op_8xy1(x, y), // OR Vx, Vy
            (0x8, _, _, 0x2) => self.op_8xy2(x, y), // AND Vx, Vy
            (0x8, _, _, 0x3) => self.op_8xy3(x, y), // XOR Vx, Vy
            (0x8, _, _, 0x4) => self.op_8xy4(x, y), // ADD Vx, Vy
            (0x8, _, _, 0x5) => self.op_8xy5(x, y), // SUB Vx, Vy
            (0x8, _, _, 0x6) => self.op_8xy6(x, y), // SHR Vx {, Vy}
            (0x8, _, _, 0x7) => self.op_8xy7(x, y), // SUBN Vx, Vy
            (0x8, _, _, 0xE) => self.op_8xye(x, y), // SHL Vx {, Vy}

            // --- Offset/Random/Display ---
            (0x9, _, _, 0x0) => self.op_9xy0(x, y), // SNE Vx, Vy
            (0xA, _, _, _) => self.op_annn(nnn),    // LD I, addr
            (0xB, _, _, _) => self.op_bnnn(nnn),    // JP V0, addr
            (0xC, _, _, _) => self.op_cxnn(x, nn),  // RND Vx, byte
            (0xD, _, _, n) => self.op_dxyn(x, y, n), // DRW Vx, Vy, nibble

            // --- E Series (Input) ---
            (0xE, _, 0x9, 0xE) => self.op_ex9e(x), // SKP Vx
            (0xE, _, 0xA, 0x1) => self.op_exa1(x), // SKNP Vx

            // --- F Series (Misc/Memory) ---
            (0xF, _, 0x0, 0x7) => self.op_fx07(x), // LD Vx, DT
            (0xF, _, 0x0, 0xA) => self.op_fx0a(x), // LD Vx, K
            (0xF, _, 0x1, 0x5) => self.op_fx15(x), // LD DT, Vx
            (0xF, _, 0x1, 0x8) => self.op_fx18(x), // LD ST, Vx
            (0xF, _, 0x1, 0xE) => self.op_fx1e(x), // ADD I, Vx
            (0xF, _, 0x2, 0x9) => self.op_fx29(x), // LD F, Vx
            (0xF, _, 0x3, 0x3) => self.op_fx33(x), // LD B, Vx
            (0xF, _, 0x5, 0x5) => self.op_fx55(x), // LD [I], Vx
            (0xF, _, 0x6, 0x5) => self.op_fx65(x), // LD Vx, [I]

            _ => println!("Unknown Opcode: {:#06x}", opcode),
        }
    }
    pub fn debug_render_console(&self) {
        // Clear console (ANSI escape code)
        print!("{}[2J", 27 as char);

        for y in 0..32 {
            for x in 0..64 {
                let pixel = self.display[x + y * 64];
                // Use a block character for 'on' and a space for 'off'
                print!("{}", if pixel == 1 { "█" } else { " " });
            }
            println!();
        }
    }
    // --- 0 Series: System and Control ---
    fn op_00e0(&mut self) {
        // CLS: Clear the display
        self.display.fill(0);
        self.draw_flag = true;
    }

    fn op_00ee(&mut self) {
        // RET: Return from a subroutine
        self.sp -= 1;
        self.pc = self.stack[self.sp as usize];
    }

    fn op_0nnn(&mut self, _addr: u16) {
        // SYS addr: Execute machine language routine (Usually ignored)
    }

    // --- 1 to 5 Series: Flow and Basic Logic ---
    fn op_1nnn(&mut self, addr: u16) {
        // JP addr: Jump to address NNN
        self.pc = addr;
    }

    fn op_2nnn(&mut self, addr: u16) {
        // CALL addr: Call subroutine at NNN
        self.stack[self.sp as usize] = self.pc; //store current address
        self.sp += 1;
        self.pc = addr;
    }

    fn op_3xnn(&mut self, x: usize, nn: u8) {
        // SE Vx, byte: Skip next instruction if Vx == NN
        if self.vx[x] == nn {
            self.pc += 2;
        }
    }

    fn op_4xnn(&mut self, x: usize, nn: u8) {
        // SNE Vx, byte: Skip next instruction if Vx != NN
        if self.vx[x] != nn {
            self.pc += 2;
        }
    }

    fn op_5xy0(&mut self, x: usize, y: usize) {
        // SE Vx, Vy: Skip next instruction if Vx == Vy
        if self.vx[x] == self.vx[y] {
            self.pc += 2;
        }
    }

    fn op_6xnn(&mut self, x: usize, nn: u8) {
        // LD Vx, byte: Set Vx = NN
        self.vx[x] = nn;
    }

    fn op_7xnn(&mut self, x: usize, nn: u8) {
        // ADD Vx, byte: Set Vx = Vx + NN (No Carry Flag)
        self.vx[x] = self.vx[x].wrapping_add(nn);
    }

    // --- 8 Series: Arithmetic and Bitwise ---
    fn op_8xy0(&mut self, x: usize, y: usize) {
        // LD Vx, Vy: Set Vx = Vy
        self.vx[x] = self.vx[y];
    }

    fn op_8xy1(&mut self, x: usize, y: usize) {
        // OR Vx, Vy: Set Vx = Vx OR Vy
        self.vx[x] |= self.vx[y];
    }

    fn op_8xy2(&mut self, x: usize, y: usize) {
        // AND Vx, Vy: Set Vx = Vx AND Vy
        self.vx[x] &= self.vx[y];
    }

    fn op_8xy3(&mut self, x: usize, y: usize) {
        // XOR Vx, Vy: Set Vx = Vx XOR Vy
        self.vx[x] ^= self.vx[y];
    }

    fn op_8xy4(&mut self, x: usize, y: usize) {
        // 1. Calculate the sum in a wider type (u16) to detect overflow
        let val_x = self.vx[x] as u16;
        let val_y = self.vx[y] as u16;
        let sum = val_x + val_y;

        // 2. Determine the carry flag (1 if it exceeds 255, else 0)
        let carry = if sum > 0xFF { 1 } else { 0 };

        // 3. Set the register (it will automatically take the lower 8 bits)
        self.vx[x] = (sum & 0xFF) as u8;

        // 4. Set the carry flag in VF (register 15)
        self.vx[0xF] = carry;
    }

    fn op_8xy5(&mut self, x: usize, y: usize) {
        // SUB Vx, Vy: Set Vx = Vx - Vy, set VF = NOT borrow
        self.vx[0xF] = if self.vx[x] >= self.vx[y] { 1 } else { 0 };
        self.vx[x] = self.vx[x].wrapping_sub(self.vx[y]);
    }

    fn op_8xy6(&mut self, x: usize, _y: usize) {
        // SHR: Set VF to the least significant bit, then shift Vx right by 1
        self.vx[0xF] = self.vx[x] & 0x1;
        self.vx[x] >>= 1;
    }

    fn op_8xy7(&mut self, x: usize, y: usize) {
        // SUBN Vx, Vy: Set Vx = Vy - Vx, set VF = NOT borrow
        self.vx[0xF] = if self.vx[y] >= self.vx[x] { 1 } else { 0 };
        self.vx[x] = self.vx[y].wrapping_sub(self.vx[x]);
    }
    fn op_8xye(&mut self, x: usize, _y: usize) {
        // SHL: Set VF to the most significant bit, then shift Vx left by 1
        self.vx[0xF] = (self.vx[x] & 0x80) >> 7;
        self.vx[x] <<= 1;
    }
    // --- 9 to D Series: Offsets, Random, and Graphics ---
    fn op_9xy0(&mut self, x: usize, y: usize) {
        // SNE Vx, Vy: Skip next instruction if Vx != Vy
        if self.vx[x] != self.vx[y] {
            self.pc += 2;
        }
    }

    fn op_annn(&mut self, addr: u16) {
        // LD I, addr: Set I = NNN
        self.i = addr;
    }

    fn op_bnnn(&mut self, addr: u16) {
        // JP V0, addr: Jump to location NNN + V0
        self.pc = addr + self.vx[0] as u16;
    }

    fn op_cxnn(&mut self, x: usize, nn: u8) {
        // RND Vx, byte: Set Vx = random byte AND NN
        let mut rng = rand::rng();
        let random_byte: u8 = rng.random();
        self.vx[x] = random_byte & nn;
    }

    fn op_dxyn(&mut self, x_idx: usize, y_idx: usize, height: u8) {
        let x_coord = (self.vx[x_idx] % 64) as usize;
        let y_coord = (self.vx[y_idx] % 32) as usize;

        let height = height as usize;
        self.vx[0xF] = 0; // Reset collision flag

        for row in 0..height {
            // Wrap the Y coordinate for the current row
            let current_y = (y_coord + row) % 32;
            let sprite_byte = self.ram[self.i as usize + row];

            for col in 0..8 {
                // Wrap the X coordinate for the current column
                let current_x = (x_coord + col) % 64;

                let mask = 0x80 >> col;

                //check if pixel in sprite is on
                if (sprite_byte & mask) != 0 {
                    let screen_idx = current_x + (current_y * 64);

                    // Collision detection: if the screen pixel is already 1
                    if self.display[screen_idx] == 1 {
                        self.vx[0xF] = 1;
                    }

                    // XOR the pixel onto the screen
                    self.display[screen_idx] ^= 1;
                }
            }
        }
        self.draw_flag = true;
    }
    // --- E Series: Input ---
    fn op_ex9e(&mut self, x: usize) {
        // SKP Vx: Skip next instruction if key with the value of Vx is pressed
        if self.keypad[self.vx[x] as usize] {
            self.pc += 2;
        }
    }

    fn op_exa1(&mut self, x: usize) {
        // SKNP Vx: Skip next instruction if key with the value of Vx is not pressed
        if !self.keypad[self.vx[x] as usize] {
            self.pc += 2;
        }
    }

    // --- F Series: Timers, Keyboard, and Memory ---
    fn op_fx07(&mut self, x: usize) {
        // LD Vx, DT: Set Vx = delay timer value
        self.vx[x] = self.delay_timer;
    }

    fn op_fx0a(&mut self, x: usize) {
        // LD Vx, K: Wait for a key press, store the value of the key in Vx
        let mut key_pressed = false;
        for i in 1..self.keypad.len() {
            if self.keypad[i] {
                self.vx[x] = i as u8;
                key_pressed = true;
                break;
            }
        }
        if !key_pressed {
            self.pc -= 2; //this causes this instruction to play again and again effectively waiting for key press on this instruction
        }
    }

    fn op_fx15(&mut self, x: usize) {
        // LD DT, Vx: Set delay timer = Vx
        self.delay_timer = self.vx[x];
    }

    fn op_fx18(&mut self, x: usize) {
        // LD ST, Vx: Set sound timer = Vx
        self.sound_timer = self.vx[x];
    }

    fn op_fx1e(&mut self, x: usize) {
        // ADD I, Vx: Set I = I + Vx
        self.i = self.i + self.vx[x] as u16;
    }

    fn op_fx29(&mut self, x: usize) {
        // LD F, Vx: Set I = location of sprite for digit Vx
        let character = self.vx[x] as u16; //we have character loaded into memory FONT SET so vx contains the number that is to be printed
        self.i = FONT_START_ADDR as u16 + (character * 5); //since every character is 5 bytes long
    }

    fn op_fx33(&mut self, x: usize) {
        // LD B, Vx: Store BCD representation of Vx in memory locations I, I+1, and I+2
        //it is for showing scores like exact number 165
        //since we cant extract out individual bits as we require division and chip8 has no division
        let value = self.vx[x];
        //get individual digit
        self.ram[self.i as usize] = value / 100; // first digit
        self.ram[self.i as usize + 1] = (value / 10) % 10; // second digit
        self.ram[self.i as usize + 2] = value % 10; // last digit
    }

    fn op_fx55(&mut self, x: usize) {
        // LD [I], Vx: Store registers V0 through Vx in memory starting at location I
        for i in 0..=x {
            self.ram[self.i as usize + i] = self.vx[i];
        }
    }

    fn op_fx65(&mut self, x: usize) {
        // LD Vx, [I]: Read registers V0 through Vx from memory starting at location I
        for i in 0..=x {
            self.vx[i] = self.ram[self.i as usize + i];
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Chip8 {
    pub fn update_keypad(&mut self, window: &Window) {
        // update our keypad buffer position based on key press
        self.keypad[0x1] = window.is_key_down(Key::W); //player 1 up
        self.keypad[0x2] = window.is_key_down(Key::Key2);
        self.keypad[0x3] = window.is_key_down(Key::Key3);
        self.keypad[0xC] = window.is_key_down(Key::K); // player 2 up

        self.keypad[0x4] = window.is_key_down(Key::Q); //player 1 down
        self.keypad[0x5] = window.is_key_down(Key::W);
        self.keypad[0x6] = window.is_key_down(Key::E);
        self.keypad[0xD] = window.is_key_down(Key::J); //player 2 down

        self.keypad[0x7] = window.is_key_down(Key::A);
        self.keypad[0x8] = window.is_key_down(Key::S);
        self.keypad[0x9] = window.is_key_down(Key::D);
        self.keypad[0xE] = window.is_key_down(Key::F);

        self.keypad[0xA] = window.is_key_down(Key::Z);
        self.keypad[0x0] = window.is_key_down(Key::X);
        self.keypad[0xB] = window.is_key_down(Key::C);
        self.keypad[0xF] = window.is_key_down(Key::V);
    }

    pub fn run(&mut self, window: &mut Window, sound: &mut Sink) {
        assert!(self.pc >= 0x200);

        // Limit the window to 60 FPS for the timers
        window.set_target_fps(60);

        while window.is_open() && !window.is_key_down(Key::Escape) {
            // 1. Update Keypad state
            self.update_keypad(&window);

            // 2. Run multiple CPU cycles per frame
            // (At 60 FPS, 10 cycles per frame = 600Hz)
            for _ in 0..10 {
                let opcode = self.fetch();
                self.decode_execute(opcode);
            }

            // 3. Update Timers (Once per frame)
            self.tick_timers();
            if self.sound_timer > 0 {
                sound.play();
            } else {
                sound.pause();
            }

            // 4. Update Window Buffer
            // minifb expects a Vec<u32> where each u32 is 0x00RRGGBB
            if self.draw_flag {
                let buffer: Vec<u32> = self
                    .display
                    .iter()
                    .map(|&p| if p == 1 { 0xFFFFFF } else { 0x000000 })
                    .collect();
                window.update_with_buffer(&buffer, 64, 32).expect("Failed to update display");
                self.draw_flag = false;
            }
            window.update();
        }
    }
}
