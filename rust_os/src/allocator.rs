use core::alloc::{GlobalAlloc, Layout};
use spin::Mutex;

const HEAP_SIZE: usize = 32 * 1024 * 1024; // 32MB Heap

#[repr(align(16))]
struct HeapMemory {
    mem: [u8; HEAP_SIZE],
}

static mut HEAP_MEM: HeapMemory = HeapMemory {
    mem: [0; HEAP_SIZE],
};

#[repr(C, align(16))]
struct Header {
    size: usize,
    free: bool,
}

struct FirstFitAllocator {
    initialized: bool,
}

impl FirstFitAllocator {
    const fn new() -> Self {
        Self { initialized: false }
    }

    unsafe fn init(&mut self) {
        if self.initialized {
            return;
        }
        let heap_ptr = unsafe { &raw mut HEAP_MEM.mem as *mut u8 };
        let header_ptr = heap_ptr as *mut Header;
        
        let header = Header {
            size: HEAP_SIZE - core::mem::size_of::<Header>(),
            free: true,
        };
        unsafe {
            header_ptr.write(header);
        }
        self.initialized = true;
    }
}

pub struct LockedAllocator {
    inner: Mutex<FirstFitAllocator>,
}

impl LockedAllocator {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(FirstFitAllocator::new()),
        }
    }

    pub unsafe fn init(&self) {
        unsafe {
            self.inner.lock().init();
        }
    }
}

#[global_allocator]
pub static ALLOCATOR: LockedAllocator = LockedAllocator::new();

unsafe impl GlobalAlloc for LockedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.inner.lock();
        if !allocator.initialized {
            unsafe {
                allocator.init();
            }
        }

        // Align size to a multiple of 16
        let header_size = core::mem::size_of::<Header>();
        let align = 16;
        let size = (layout.size() + align - 1) & !(align - 1);

        let heap_start = unsafe { &raw mut HEAP_MEM.mem as *mut u8 };
        let heap_end = unsafe { heap_start.add(HEAP_SIZE) };
        
        let mut current = heap_start;
        while current < heap_end {
            let header = unsafe { &mut *(current as *mut Header) };
            if header.size == 0 {
                // Should not happen, prevent infinite loop
                break;
            }
            if header.free && header.size >= size {
                // Found a free block large enough
                let remaining = header.size - size;
                if remaining >= header_size + 16 {
                    // Split the block
                    let next_block_ptr = unsafe { current.add(header_size).add(size) };
                    let next_header = Header {
                        size: remaining - header_size,
                        free: true,
                    };
                    unsafe {
                        (next_block_ptr as *mut Header).write(next_header);
                    }
                    header.size = size;
                }
                header.free = false;
                return unsafe { current.add(header_size) };
            }
            // Move to the next block
            current = unsafe { current.add(header_size).add(header.size) };
        }

        core::ptr::null_mut() // Out of memory
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }
        let _allocator = self.inner.lock();
        let header_size = core::mem::size_of::<Header>();
        let header_ptr = unsafe { ptr.sub(header_size) as *mut Header };
        let header = unsafe { &mut *header_ptr };
        header.free = true;

        // Coalesce adjacent free blocks
        let heap_start = unsafe { &raw mut HEAP_MEM.mem as *mut u8 };
        let heap_end = unsafe { heap_start.add(HEAP_SIZE) };
        
        let mut current = heap_start;
        while current < heap_end {
            let curr_header = unsafe { &mut *(current as *mut Header) };
            if curr_header.size == 0 {
                break;
            }
            let next_ptr = unsafe { current.add(header_size).add(curr_header.size) };
            if next_ptr >= heap_end {
                break;
            }
            let next_header = unsafe { &mut *(next_ptr as *mut Header) };
            if curr_header.free && next_header.free {
                // Merge with the next block
                curr_header.size += header_size + next_header.size;
                // Do not increment current; try to merge again
                continue;
            }
            current = next_ptr;
        }
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
