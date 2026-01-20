import init, { Chip8Wasm } from "../pkg/chip8_emulator.js";

const WIDTH = 64;
const HEIGHT = 32;

const canvas = document.getElementById("screen");
const ctx = canvas.getContext("2d", { alpha: false });
ctx.imageSmoothingEnabled = false;

const imageData = ctx.createImageData(WIDTH, HEIGHT);
const keyMap = {
  KeyW: 0x1,
  KeyQ: 0x4,
  KeyK: 0xC,
  KeyJ: 0xD,
};

let chip8 = null;

function handleKey(event, pressed) {
  if (!chip8) {
    return;
  }
  const key = keyMap[event.code];
  if (key === undefined) {
    return;
  }
  event.preventDefault();
  chip8.set_key(key, pressed);
}

window.addEventListener("keydown", (event) => handleKey(event, true));
window.addEventListener("keyup", (event) => handleKey(event, false));

function render(frameBuffer) {
  for (let i = 0; i < frameBuffer.length; i++) {
    const value = frameBuffer[i] ? 255 : 0;
    const base = i * 4;
    imageData.data[base] = value;
    imageData.data[base + 1] = value;
    imageData.data[base + 2] = value;
    imageData.data[base + 3] = 255;
  }
  ctx.putImageData(imageData, 0, 0);
}

function loop() {
  chip8.tick();
  render(chip8.frame());
  requestAnimationFrame(loop);
}

async function start() {
  await init();
  chip8 = new Chip8Wasm();
  chip8.load_pong();
  loop();
}

start();
