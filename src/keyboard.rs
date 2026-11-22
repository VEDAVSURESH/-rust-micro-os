//! PS/2 Keyboard Driver & Input Buffer for JainOS.
//!
//! Handles IRQ 1 interrupts (vector 33), reads Scancode Set 1 bytes from port 0x60,
//! decodes scancodes with modifier tracking (Shift, CapsLock, Ctrl, Alt),
//! and enqueues decoded characters into a thread-safe ring buffer.

use crate::serial::inb;
use crate::sync::Spinlock;

/// Capacity of the keyboard FIFO input ring buffer.
const BUFFER_CAPACITY: usize = 256;

/// Thread-safe circular FIFO buffer for keyboard character input.
pub struct KeyboardBuffer {
    data: [char; BUFFER_CAPACITY],
    head: usize,
    tail: usize,
    count: usize,
}

impl KeyboardBuffer {
    pub const fn new() -> Self {
        Self {
            data: ['\0'; BUFFER_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    /// Pushes a character into the FIFO buffer.
    /// If full, the oldest unread character is overwritten to prevent blocking interrupts.
    pub fn push(&mut self, c: char) {
        if self.count == BUFFER_CAPACITY {
            // Buffer full: advance head to discard oldest byte
            self.head = (self.head + 1) % BUFFER_CAPACITY;
            self.count -= 1;
        }
        self.data[self.tail] = c;
        self.tail = (self.tail + 1) % BUFFER_CAPACITY;
        self.count += 1;
    }

    /// Pops a character from the buffer.
    pub fn pop(&mut self) -> Option<char> {
        if self.count == 0 {
            None
        } else {
            let c = self.data[self.head];
            self.head = (self.head + 1) % BUFFER_CAPACITY;
            self.count -= 1;
            Some(c)
        }
    }

    /// Returns true if the buffer has available characters to read.
    pub fn has_data(&self) -> bool {
        self.count > 0
    }

    /// Returns the number of characters available in the buffer.
    pub fn len(&self) -> usize {
        self.count
    }
}

/// Global synchronized keyboard input buffer.
pub static KEYBOARD_BUFFER: Spinlock<KeyboardBuffer> = Spinlock::new(KeyboardBuffer::new());

/// Decoder state for PS/2 Scancode Set 1.
struct KeyboardDecoder {
    shift_pressed: bool,
    caps_lock: bool,
    ctrl_pressed: bool,
    alt_pressed: bool,
    extended: bool,
}

impl KeyboardDecoder {
    const fn new() -> Self {
        Self {
            shift_pressed: false,
            caps_lock: false,
            ctrl_pressed: false,
            alt_pressed: false,
            extended: false,
        }
    }

    /// Processes a single raw scancode from port 0x60.
    /// Returns a decoded character if a printable key, Enter, or Backspace was pressed.
    fn process_scancode(&mut self, scancode: u8) -> Option<char> {
        // Extended scancode prefix (0xE0)
        if scancode == 0xE0 {
            self.extended = true;
            return None;
        }

        let is_release = (scancode & 0x80) != 0;
        let code = scancode & 0x7F;

        if is_release {
            match code {
                0x2A | 0x36 => self.shift_pressed = false,
                0x1D => self.ctrl_pressed = false,
                0x38 => self.alt_pressed = false,
                _ => {}
            }
            self.extended = false;
            return None;
        }

        // Key Press
        let mut result = None;

        match code {
            // Shift keys
            0x2A | 0x36 => self.shift_pressed = true,
            // Ctrl
            0x1D => self.ctrl_pressed = true,
            // Alt
            0x38 => self.alt_pressed = true,
            // CapsLock toggle
            0x3A => self.caps_lock = !self.caps_lock,
            // Enter
            0x1C => result = Some('\n'),
            // Backspace
            0x0E => result = Some('\x08'),
            // Tab
            0x0F => result = Some('\t'),
            // Space
            0x39 => result = Some(' '),
            // Esc
            0x01 => result = Some('\x1B'),

            // Numbers row
            0x02 => result = Some(if self.shift_pressed { '!' } else { '1' }),
            0x03 => result = Some(if self.shift_pressed { '@' } else { '2' }),
            0x04 => result = Some(if self.shift_pressed { '#' } else { '3' }),
            0x05 => result = Some(if self.shift_pressed { '$' } else { '4' }),
            0x06 => result = Some(if self.shift_pressed { '%' } else { '5' }),
            0x07 => result = Some(if self.shift_pressed { '^' } else { '6' }),
            0x08 => result = Some(if self.shift_pressed { '&' } else { '7' }),
            0x09 => result = Some(if self.shift_pressed { '*' } else { '8' }),
            0x0A => result = Some(if self.shift_pressed { '(' } else { '9' }),
            0x0B => result = Some(if self.shift_pressed { ')' } else { '0' }),
            0x0C => result = Some(if self.shift_pressed { '_' } else { '-' }),
            0x0D => result = Some(if self.shift_pressed { '+' } else { '=' }),

            // Top letter row
            0x10 => result = Some(self.letter('q', 'Q')),
            0x11 => result = Some(self.letter('w', 'W')),
            0x12 => result = Some(self.letter('e', 'E')),
            0x13 => result = Some(self.letter('r', 'R')),
            0x14 => result = Some(self.letter('t', 'T')),
            0x15 => result = Some(self.letter('y', 'Y')),
            0x16 => result = Some(self.letter('u', 'U')),
            0x17 => result = Some(self.letter('i', 'I')),
            0x18 => result = Some(self.letter('o', 'O')),
            0x19 => result = Some(self.letter('p', 'P')),
            0x1A => result = Some(if self.shift_pressed { '{' } else { '[' }),
            0x1B => result = Some(if self.shift_pressed { '}' } else { ']' }),

            // Home letter row
            0x1E => result = Some(self.letter('a', 'A')),
            0x1F => result = Some(self.letter('s', 'S')),
            0x20 => result = Some(self.letter('d', 'D')),
            0x21 => result = Some(self.letter('f', 'F')),
            0x22 => result = Some(self.letter('g', 'G')),
            0x23 => result = Some(self.letter('h', 'H')),
            0x24 => result = Some(self.letter('j', 'J')),
            0x25 => result = Some(self.letter('k', 'K')),
            0x26 => result = Some(self.letter('l', 'L')),
            0x27 => result = Some(if self.shift_pressed { ':' } else { ';' }),
            0x28 => result = Some(if self.shift_pressed { '"' } else { '\'' }),
            0x29 => result = Some(if self.shift_pressed { '~' } else { '`' }),
            0x2B => result = Some(if self.shift_pressed { '|' } else { '\\' }),

            // Bottom letter row
            0x2C => result = Some(self.letter('z', 'Z')),
            0x2D => result = Some(self.letter('x', 'X')),
            0x2E => result = Some(self.letter('c', 'C')),
            0x2F => result = Some(self.letter('v', 'V')),
            0x30 => result = Some(self.letter('b', 'B')),
            0x31 => result = Some(self.letter('n', 'N')),
            0x32 => result = Some(self.letter('m', 'M')),
            0x33 => result = Some(if self.shift_pressed { '<' } else { ',' }),
            0x34 => result = Some(if self.shift_pressed { '>' } else { '.' }),
            0x35 => result = Some(if self.shift_pressed { '?' } else { '/' }),

            // Keypad / NumPad basic operators
            0x4A => result = Some('-'),
            0x4E => result = Some('+'),
            0x37 => result = Some('*'),

            _ => {}
        }

        self.extended = false;
        result
    }

    #[inline]
    fn letter(&self, lower: char, upper: char) -> char {
        if self.shift_pressed ^ self.caps_lock {
            upper
        } else {
            lower
        }
    }
}

static DECODER: Spinlock<KeyboardDecoder> = Spinlock::new(KeyboardDecoder::new());

/// Invoked directly from the IRQ 1 interrupt handler.
/// Reads the scancode from port 0x60, decodes it, and pushes any resulting character.
pub fn handle_interrupt() {
    let scancode = unsafe { inb(0x60) };
    if let Some(c) = DECODER.lock().process_scancode(scancode) {
        KEYBOARD_BUFFER.lock().push(c);
    }
}

/// Reads a character from the keyboard buffer if available, or serial console. Non-blocking.
pub fn read_char() -> Option<char> {
    if let Some(c) = KEYBOARD_BUFFER.lock().pop() {
        return Some(c);
    }
    // Also accept input from serial port (COM1) for headless terminal automation
    if let Some(byte) = crate::serial::SERIAL1.lock().receive_byte() {
        let c = match byte {
            b'\r' => '\n',
            0x7F | 0x08 => '\x08',
            b => b as char,
        };
        return Some(c);
    }
    None
}

/// Reads a character from the keyboard buffer, waiting (yielding) until one is available.
pub fn get_char_blocking() -> char {
    loop {
        if let Some(c) = read_char() {
            return c;
        }
        crate::task::yield_now();
        x86_64::instructions::hlt();
    }
}
