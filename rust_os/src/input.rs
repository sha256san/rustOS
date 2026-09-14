const BUFFER_SIZE: usize = 128;

pub struct RingBuffer {
    data: [u8; BUFFER_SIZE],
    head: usize,
    tail: usize,
}

impl RingBuffer {
    pub const fn new() -> Self {
        RingBuffer {
            data: [0; BUFFER_SIZE],
            head: 0,
            tail: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    pub fn is_full(&self) -> bool {
        (self.tail + 1) % BUFFER_SIZE == self.head
    }

    pub fn push(&mut self, val: u8) -> Result<(), ()> {
        if self.is_full() {
            return Err(());
        }
        self.data[self.tail] = val;
        self.tail = (self.tail + 1) % BUFFER_SIZE;
        Ok(())
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.is_empty() {
            return None;
        }
        let val = self.data[self.head];
        self.head = (self.head + 1) % BUFFER_SIZE;
        Some(val)
    }
}

pub static INPUT_BUFFER: spin::Mutex<RingBuffer> = spin::Mutex::new(RingBuffer::new());

pub fn add_char(c: u8) {
    let mut buffer = INPUT_BUFFER.lock();
    let _ = buffer.push(c);
}

pub fn read_char() -> Option<u8> {
    let mut buffer = INPUT_BUFFER.lock();
    buffer.pop()
}
