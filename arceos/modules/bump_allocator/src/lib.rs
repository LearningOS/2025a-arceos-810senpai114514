#![no_std]

use allocator::{BaseAllocator, ByteAllocator, PageAllocator};

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const SIZE: usize> {
    start: usize,
    size: usize,
    b_pos: usize,
    p_pos: usize,
    count: usize,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            size: 0,
            b_pos: 0,
            p_pos: 0,
            count: 0,
        }
    }
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.size = size;
        self.b_pos = start;
        self.p_pos = start + size;
        self.count = 0;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> allocator::AllocResult {
        if start + size > self.start + self.size {
            return Err(allocator::AllocError::NoMemory);
        }
        self.p_pos = start + size;
        Ok(())
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(
        &mut self,
        layout: core::alloc::Layout,
    ) -> allocator::AllocResult<core::ptr::NonNull<u8>> {
        let size = layout.size();
        let align = layout.align();
        
        // Calculate aligned position
        let aligned_pos = (self.b_pos + align - 1) & !(align - 1);
        
        // Check if we have enough space
        if aligned_pos + size > self.p_pos {
            return Err(allocator::AllocError::NoMemory);
        }
        
        // Update b_pos and count
        self.b_pos = aligned_pos + size;
        self.count += 1;
        
        // Return the aligned pointer
        Ok(core::ptr::NonNull::new(aligned_pos as *mut u8).unwrap())
    }

    fn dealloc(&mut self, _pos: core::ptr::NonNull<u8>, _layout: core::alloc::Layout) {
        // Decrement count
        if self.count > 0 {
            self.count -= 1;
        }
        
        // If count reaches zero, reset b_pos to start
        if self.count == 0 {
            self.b_pos = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        self.size
    }

    fn used_bytes(&self) -> usize {
        self.b_pos - self.start
    }

    fn available_bytes(&self) -> usize {
        self.p_pos - self.b_pos
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(
        &mut self,
        num_pages: usize,
        align_pow2: usize,
    ) -> allocator::AllocResult<usize> {
        let required_bytes = num_pages * SIZE;
        
        // Check if we have enough space
        if required_bytes > self.p_pos - self.b_pos {
            return Err(allocator::AllocError::NoMemory);
        }
        
        // Calculate aligned position (aligning backward from p_pos)
        let unaligned_pos = self.p_pos - required_bytes;
        let aligned_pos = unaligned_pos & !(align_pow2 - 1);
        
        // Check if aligned position doesn't overlap with b_pos
        if aligned_pos < self.b_pos {
            return Err(allocator::AllocError::NoMemory);
        }
        
        // Update p_pos
        self.p_pos = aligned_pos;
        
        Ok(aligned_pos)
    }

    fn dealloc_pages(&mut self, pos: usize, num_pages: usize) {
        let required_bytes = num_pages * SIZE;
        if pos + required_bytes > self.p_pos {
            return;
        }
        self.p_pos = pos + required_bytes;
    }

    fn total_pages(&self) -> usize {
        self.size / SIZE
    }

    fn used_pages(&self) -> usize {
        (self.p_pos - self.start) / SIZE
    }

    fn available_pages(&self) -> usize {
        (self.p_pos - self.b_pos) / SIZE
    }
}