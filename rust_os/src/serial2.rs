use x86_64::instructions::port::Port;

pub struct Serial2Port {
    data: Port<u8>,
    line_status: Port<u8>,
}

impl Serial2Port {
    pub const fn new() -> Self {
        Self {
            data: Port::new(0x2f8),
            line_status: Port::new(0x2f8 + 5),
        }
    }

    pub fn init(&mut self) {
        unsafe {
            // Disable interrupts
            Port::new(0x2f8 + 1).write(0x00_u8);
            // Enable DLAB (set baud rate divisor)
            Port::new(0x2f8 + 3).write(0x80_u8);
            // Set divisor to 1 (115200 baud)
            Port::new(0x2f8 + 0).write(0x01_u8);
            Port::new(0x2f8 + 1).write(0x00_u8);
            // 8 bits, no parity, one stop bit
            Port::new(0x2f8 + 3).write(0x03_u8);
            // Enable FIFO, clear them, with 14-byte threshold
            Port::new(0x2f8 + 2).write(0xC7_u8);
            // RTS/DSR set
            Port::new(0x2f8 + 4).write(0x0B_u8);
        }
    }

    fn is_transmit_empty(&mut self) -> bool {
        unsafe { self.line_status.read() & 0x20 != 0 }
    }

    pub fn send_byte(&mut self, byte: u8) {
        while !self.is_transmit_empty() {}
        unsafe {
            self.data.write(byte);
        }
    }

    fn is_data_ready(&mut self) -> bool {
        unsafe { self.line_status.read() & 1 != 0 }
    }

    pub fn read_byte(&mut self) -> u8 {
        while !self.is_data_ready() {}
        unsafe {
            self.data.read()
        }
    }

    pub fn send_string(&mut self, s: &str) {
        for byte in s.bytes() {
            self.send_byte(byte);
        }
    }
}

pub static SERIAL2: spin::Mutex<Serial2Port> = spin::Mutex::new(Serial2Port::new());
