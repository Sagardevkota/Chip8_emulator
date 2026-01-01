use chip8_emulator::Chip8;
use minifb::{Window, WindowOptions};
use rodio::{source::SineWave, OutputStream, Sink, Source};
use std::fs;

fn main() {
    let mut chip8 = Chip8::new();
    let contents = fs::read("roms/Pong.ch8").expect("Could not read rom file");
    chip8.load_rom(&contents);
    let mut window = Window::new(
        "Chip-8 Emulator",
        64,
        32, // Internal resolution
        WindowOptions {
            scale: minifb::Scale::X16, // Scale 64x32 up to 1024x512
            ..WindowOptions::default()
        },
    )
    .expect("Failed to create window");
    let (_stream, stream_handle) = OutputStream::try_default().expect("Failed to get audio output");
    let mut sound = create_sound(&stream_handle);
    chip8.run(&mut window, &mut sound);
}

fn create_sound(handle: &rodio::OutputStreamHandle) -> Sink {
    let sink = Sink::try_new(handle).expect("Failed to create audio sink");
    let source = SineWave::new(440.0)
        .amplify(0.2)
        .repeat_infinite();
    sink.append(source);
    sink.pause();
    sink
}

#[cfg(test)]
mod tests {
    use crate::Chip8;

    #[test]
    fn test_fetch() {
        let mut chip8 = Chip8::new();
        let rom_data: [u8; 4] = [0x12, 0x34, 0x56, 0x78];
        chip8.load_rom(&rom_data);
        let opcode = chip8.fetch();
        assert_eq!(opcode, 0x1234);
        assert_eq!(chip8.pc, 0x202);
    }
    #[test]
    fn test_arithmetic_and_carry_flag() {
        let mut chip8 = Chip8::new();

        // 1. Load the test program
        // 0x61C8: V1 = 200
        // 0x6264: V2 = 100
        // 0x710A: V1 += 10
        // 0x8124: V1 += V2 (This should trigger carry)
        let program: [u8; 8] = [0x61, 0xC8, 0x62, 0x64, 0x71, 0x0A, 0x81, 0x24];
        chip8.load_rom(&program);

        // Instruction 1: 0x61C8 (LD V1, 200)
        let op = chip8.fetch();
        chip8.decode_execute(op);
        assert_eq!(chip8.vx[1], 200);

        // Instruction 2: 0x6264 (LD V2, 100)
        let op = chip8.fetch();
        chip8.decode_execute(op);
        assert_eq!(chip8.vx[2], 100);

        // Instruction 3: 0x710A (ADD V1, 10)
        let op = chip8.fetch();
        chip8.decode_execute(op);
        assert_eq!(chip8.vx[1], 210);
        assert_eq!(chip8.vx[0xF], 0, "7XNN should not affect VF");

        // Instruction 4: 0x8124 (ADD V1, V2)
        // 210 + 100 = 310. Result should be 310 - 256 = 54. VF should be 1.
        let op = chip8.fetch();
        chip8.decode_execute(op);

        assert_eq!(chip8.vx[1], 54, "V1 should overflow and wrap to 54");
        assert_eq!(chip8.vx[0xF], 1, "VF should be 1 due to carry");
    }
    #[test]
    fn test_display_font_render() {
        let mut chip8 = Chip8::new(); // Ensure this loads the FONT_SET into RAM

        // 1. Point I to the font for '0'
        // Opcode F029: Load font for V0 into I (V0 is 0 by default)
        chip8.decode_execute(0xF029);

        // 2. Draw the 5-byte sprite at (0, 0)
        // Opcode D005: Draw from I at V0, V0, height 5
        chip8.decode_execute(0xD005);

        // 3. Verify the buffer logic for digit '0'
        // Digit '0' is 0xF0 (1111 0000) at the first row.
        // This means pixels (0,0), (1,0), (2,0), (3,0) should be 1.
        assert_eq!(chip8.display[0], 1);
        assert_eq!(chip8.display[1], 1);
        assert_eq!(chip8.display[2], 1);
        assert_eq!(chip8.display[3], 1);
        assert_eq!(chip8.display[4], 0); // Fifth pixel in row 0 should be 0

        // Digit '0' middle row (e.g., row 1) is 0x90 (1001 0000)
        // This means pixel (0,1) is 1 and (3,1) is 1.
        let row_1_offset = 1 * 64;
        assert_eq!(chip8.display[row_1_offset + 0], 1);
        assert_eq!(chip8.display[row_1_offset + 1], 0);
        assert_eq!(chip8.display[row_1_offset + 2], 0);
        assert_eq!(chip8.display[row_1_offset + 3], 1);

        // 4. Finally, visually inspect it
        println!("Visual Inspection of Digit 0:");
        chip8.debug_render_console();
    }
    #[test]
    fn test_call_and_return() {
        let mut chip8 = Chip8::new();

        // 1. Setup a mini program
        // Address 0x200: 0x2400 (CALL 0x400)
        // Address 0x202: 0x6101 (LD V1, 1 - The "Success" flag)
        // ...
        // Address 0x400: 0x00EE (RET)
        let program: [u8; 4] = [0x24, 0x00, 0x61, 0x01];
        chip8.load_rom(&program);

        // Manually place the RET instruction at 0x400
        chip8.ram[0x400] = 0x00;
        chip8.ram[0x401] = 0xEE;

        // --- Step 1: Execute CALL 0x400 ---
        let op = chip8.fetch();
        chip8.decode_execute(op);

        assert_eq!(chip8.pc, 0x400, "PC should have jumped to 0x400");
        assert_eq!(chip8.sp, 1, "Stack Pointer should be 1 after a call");
        assert_eq!(
            chip8.stack[0], 0x202,
            "Stack should store the return address (0x202)"
        );

        // --- Step 2: Execute RET at 0x400 ---
        let op = chip8.fetch(); // This fetches from 0x400
        chip8.decode_execute(op);

        assert_eq!(chip8.pc, 0x202, "PC should have returned to 0x202");
        assert_eq!(
            chip8.sp, 0,
            "Stack Pointer should be back to 0 after return"
        );

        // --- Step 3: Execute the instruction we returned to ---
        let op = chip8.fetch();
        chip8.decode_execute(op);
        assert_eq!(
            chip8.vx[1], 1,
            "Should have executed the instruction after the CALL"
        );
    }
    #[test]
    fn test_ibm_logo_logic() {
        let mut chip8 = Chip8::new();

        // Load the actual IBM Logo ROM bytes
        // You can find these online or read from a file
        let rom = [
            0x00, 0xE0, 0xA2, 0x2A, 0x60, 0x0C, 0x61, 0x08, 0xD0, 0x1F, 0x70, 0x09, 0xA2, 0x39,
            0xD0, 0x1F, 0xA2, 0x48, 0x70, 0x08, 0xD0, 0x1F, 0x12, 0x16, 0x00, 0x00, 0x7C, 0x82,
            0x82, 0x82, 0x7C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x1F, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F, 0x11,
            0x11, 0x11, 0x1F, 0x11, 0x11, 0x11, 0x1F, 0x00, 0x00, 0x00,
        ];
        chip8.load_rom(&rom);

        // Run for about 20 cycles
        for _ in 0..20 {
            let op = chip8.fetch();
            chip8.decode_execute(op);
        }

        // Print the result to your terminal!
        chip8.debug_render_console();
    }
}
