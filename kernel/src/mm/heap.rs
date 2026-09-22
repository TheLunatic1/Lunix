use linked_list_allocator::LockedHeap;

pub const HEAP_SIZE: usize = 64 * 1024 * 1024; // 64 MiB heap

static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init() {
    unsafe {
        let heap_ptr = (&raw mut HEAP_MEM) as *mut u8;
        ALLOCATOR.lock().init(heap_ptr, HEAP_SIZE);
    }
}
