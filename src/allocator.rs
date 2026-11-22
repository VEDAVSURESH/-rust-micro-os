//! Educational Linked-List Kernel Heap Allocator for JainOS.
//!
//! This module implements an in-tree first-fit coalescing linked-list allocator,
//! wrapped in JainOS's interrupt-safe `Spinlock`, implementing Rust's `GlobalAlloc` trait.
//! It provides:
//! - Strict adherence to alignment and size requirements.
//! - Address-ordered free list insertion.
//! - Instant adjacent free-block coalescing to eliminate external fragmentation.
//! - Live, exact heap diagnostics (`HeapStats`).
//! - Verified virtual page mapping of the kernel heap with non-executable permissions.

use core::alloc::{GlobalAlloc, Layout};
use crate::memory::BootInfoFrameAllocator;
use crate::sync::Spinlock;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags,
};
use x86_64::VirtAddr;

/// Candidate virtual address where the kernel heap begins.
pub const HEAP_START: usize = 0x_4444_4444_0000;

/// Kernel heap size (16 MiB, 4096 x 4 KiB pages) for native graphical desktop & buffers.
pub const HEAP_SIZE: usize = 16 * 1024 * 1024;

/// Align the given address upwards to the nearest multiple of `align` using checked arithmetic.
///
/// Returns `Some(aligned_addr)` on success, or `None` if `align` is not a power of two
/// or if an arithmetic overflow occurs.
pub fn align_up_checked(addr: usize, align: usize) -> Option<usize> {
    if !align.is_power_of_two() {
        return None;
    }
    let mask = align.checked_sub(1)?;
    let added = addr.checked_add(mask)?;
    Some(added & !mask)
}

/// Align the given address upwards to the nearest multiple of `align`.
///
/// # Panics
/// Panics if `align` is not a power of two or if an arithmetic overflow occurs.
pub fn align_up(addr: usize, align: usize) -> usize {
    align_up_checked(addr, align).expect("align_up arithmetic overflow or non-power-of-two alignment")
}

/// A node in the intrusive linked list of free heap blocks.
///
/// The header is stored directly inside the unused memory of the free block itself.
struct ListNode {
    size: usize,
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr()
            .checked_add(self.size)
            .expect("ListNode end_addr arithmetic overflow")
    }
}

/// Exact diagnostic statistics reflecting the live state of the heap allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapStats {
    /// Virtual start address of the heap.
    pub heap_start: usize,
    /// Total configured size of the heap in bytes.
    pub heap_size: usize,
    /// Current number of bytes actively allocated to users.
    pub total_allocated_bytes: usize,
    /// Current number of free bytes available in the heap.
    pub total_free_bytes: usize,
    /// Cumulative count of successful allocations.
    pub alloc_count: usize,
    /// Cumulative count of successful deallocations.
    pub dealloc_count: usize,
    /// Cumulative count of failed allocation requests (e.g. out-of-memory).
    pub alloc_failures: usize,
}

/// A linked-list allocator that manages heap memory blocks.
pub struct LinkedListAllocator {
    head: ListNode,
    heap_start: usize,
    heap_size: usize,
    total_allocated_bytes: usize,
    total_free_bytes: usize,
    alloc_count: usize,
    dealloc_count: usize,
    alloc_failures: usize,
}

impl LinkedListAllocator {
    /// Creates an uninitialized `LinkedListAllocator`.
    pub const fn new() -> Self {
        LinkedListAllocator {
            head: ListNode::new(0),
            heap_start: 0,
            heap_size: 0,
            total_allocated_bytes: 0,
            total_free_bytes: 0,
            alloc_count: 0,
            dealloc_count: 0,
            alloc_failures: 0,
        }
    }

    /// Initializes the heap with the given memory range.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the memory range `[heap_start, heap_start + heap_size)`
    /// is completely mapped to valid, writable physical frames and is unused by any other subsystem.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        assert!(
            heap_size >= core::mem::size_of::<ListNode>(),
            "Heap size too small for initial node"
        );
        assert_eq!(
            heap_start % core::mem::align_of::<ListNode>(),
            0,
            "Heap start address must be aligned to ListNode (8 bytes)"
        );
        assert_eq!(
            heap_start % 4096,
            0,
            "Heap start address must be 4 KiB page-aligned"
        );
        assert_eq!(
            heap_size % 4096,
            0,
            "Heap size must be a multiple of 4 KiB page size"
        );

        self.heap_start = heap_start;
        self.heap_size = heap_size;
        self.total_allocated_bytes = 0;
        self.total_free_bytes = heap_size;
        self.alloc_count = 0;
        self.dealloc_count = 0;
        self.alloc_failures = 0;

        // Initialize free list with a single block covering the entire heap
        let initial_node = unsafe { &mut *(heap_start as *mut ListNode) };
        initial_node.size = heap_size;
        initial_node.next = None;
        self.head.next = Some(initial_node);
    }

    /// Returns a copy of the current live heap statistics.
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            heap_start: self.heap_start,
            heap_size: self.heap_size,
            total_allocated_bytes: self.total_allocated_bytes,
            total_free_bytes: self.total_free_bytes,
            alloc_count: self.alloc_count,
            dealloc_count: self.dealloc_count,
            alloc_failures: self.alloc_failures,
        }
    }

    /// Allocates a memory block matching `layout` using the first-fit strategy with prefix/suffix splitting.
    pub fn allocate_first_fit(&mut self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        if size == 0 {
            return layout.align() as *mut u8;
        }

        // Each allocated block must be large enough to hold a ListNode when freed
        let required_size = core::cmp::max(size, core::mem::size_of::<ListNode>());
        let alloc_size = align_up(required_size, core::mem::align_of::<ListNode>());
        let alloc_align = core::cmp::max(layout.align(), core::mem::align_of::<ListNode>());

        let mut current = &mut self.head;

        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(region, alloc_size, alloc_align) {
                let mut next_node = region.next.take();
                let region_start = region.start_addr();
                let region_end = region.end_addr();
                let alloc_end = alloc_start
                    .checked_add(alloc_size)
                    .expect("alloc_end arithmetic overflow");

                // If suffix gap is large enough to hold a ListNode, split it into the free list
                let excess_suffix = region_end
                    .checked_sub(alloc_end)
                    .expect("excess_suffix underflow");
                if excess_suffix >= core::mem::size_of::<ListNode>() {
                    let suffix_node = unsafe { &mut *(alloc_end as *mut ListNode) };
                    suffix_node.size = excess_suffix;
                    suffix_node.next = next_node;
                    next_node = Some(suffix_node);
                }

                // If prefix gap is large enough to hold a ListNode, keep it in the free list
                let excess_prefix = alloc_start
                    .checked_sub(region_start)
                    .expect("excess_prefix underflow");
                if excess_prefix >= core::mem::size_of::<ListNode>() {
                    let prefix_node = unsafe { &mut *(region_start as *mut ListNode) };
                    prefix_node.size = excess_prefix;
                    prefix_node.next = next_node;
                    current.next = Some(prefix_node);
                } else {
                    current.next = next_node;
                }

                self.total_allocated_bytes = self
                    .total_allocated_bytes
                    .checked_add(alloc_size)
                    .expect("total_allocated_bytes overflow");
                self.total_free_bytes = self
                    .total_free_bytes
                    .checked_sub(alloc_size)
                    .expect("total_free_bytes underflow");
                self.alloc_count = self.alloc_count.saturating_add(1);

                return alloc_start as *mut u8;
            }

            current = current.next.as_mut().unwrap();
        }

        // Allocation failed: out of memory or fragmentation
        self.alloc_failures = self.alloc_failures.saturating_add(1);
        core::ptr::null_mut()
    }

    /// Tests whether an allocation can be carved out of `region`.
    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let region_start = region.start_addr();
        let region_end = region.end_addr();

        let alloc_start = if region_start % align == 0 {
            region_start
        } else {
            let offset_start = region_start
                .checked_add(core::mem::size_of::<ListNode>())
                .ok_or(())?;
            align_up_checked(offset_start, align).ok_or(())?
        };

        let alloc_end = alloc_start.checked_add(size).ok_or(())?;
        if alloc_end > region_end {
            return Err(());
        }

        let excess_suffix = region_end.checked_sub(alloc_end).ok_or(())?;
        if excess_suffix > 0 && excess_suffix < core::mem::size_of::<ListNode>() {
            return Err(());
        }

        let excess_prefix = alloc_start.checked_sub(region_start).ok_or(())?;
        if excess_prefix > 0 && excess_prefix < core::mem::size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }

    /// Deallocates the memory block at `ptr` and coalesces adjacent free blocks.
    pub fn deallocate(&mut self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() || layout.size() == 0 {
            return;
        }

        let new_start = ptr as usize;

        // 1. Validate that freed address satisfies ListNode alignment (8 bytes)
        assert_eq!(
            new_start % core::mem::align_of::<ListNode>(),
            0,
            "Deallocation pointer {:#x} is not aligned to ListNode (8 bytes)",
            new_start
        );

        // 2. Validate that freed address satisfies layout alignment
        assert_eq!(
            new_start % layout.align(),
            0,
            "Deallocation pointer {:#x} does not satisfy layout alignment {}",
            new_start,
            layout.align()
        );

        let required_size = core::cmp::max(layout.size(), core::mem::size_of::<ListNode>());
        let alloc_size = align_up(required_size, core::mem::align_of::<ListNode>());

        let new_end = new_start
            .checked_add(alloc_size)
            .expect("Deallocation new_end arithmetic overflow");

        // Sanity check: must lie within heap bounds
        let heap_end = self
            .heap_start
            .checked_add(self.heap_size)
            .expect("Heap bounds calculation overflow");
        assert!(
            new_start >= self.heap_start && new_end <= heap_end,
            "Heap deallocation pointer out of heap bounds: [{:#x}, {:#x}) not in [{:#x}, {:#x})",
            new_start,
            new_end,
            self.heap_start,
            heap_end
        );

        // Store head pointer before borrowing self.head mutably
        let head_ptr = &self.head as *const ListNode;
        let mut current = &mut self.head;

        while let Some(ref mut next) = current.next {
            if next.start_addr() > new_start {
                break;
            }
            current = current.next.as_mut().unwrap();
        }

        // Reliably distinguish between the allocator's dummy head and a genuine preceding heap block
        let is_preceding_block = (current as *const ListNode) != head_ptr;

        // Overlap verification to detect heap corruption and double-free
        if is_preceding_block {
            assert!(
                current.end_addr() <= new_start,
                "Heap corruption: freed block [{:#x}, {:#x}) overlaps preceding free block [{:#x}, {:#x})",
                new_start,
                new_end,
                current.start_addr(),
                current.end_addr()
            );
        }

        if let Some(ref next) = current.next {
            assert!(
                new_end <= next.start_addr(),
                "Heap corruption: freed block [{:#x}, {:#x}) overlaps following free block [{:#x}, {:#x})",
                new_start,
                new_end,
                next.start_addr(),
                next.end_addr()
            );
        }

        // Adjacent free-block coalescing
        let merged_start = new_start;
        let mut merged_end = new_end;
        let mut next_node = current.next.take();

        // 1. Coalesce with following block if adjacent
        if let Some(following) = next_node {
            if merged_end == following.start_addr() {
                merged_end = following.end_addr();
                next_node = following.next.take();
            } else {
                next_node = Some(following);
            }
        }

        // 2. Coalesce with preceding block if adjacent
        if is_preceding_block && current.end_addr() == merged_start {
            current.size = current
                .size
                .checked_add(merged_end - merged_start)
                .expect("Preceding block coalesce size overflow");
            current.next = next_node;
        } else {
            let new_node = unsafe { &mut *(merged_start as *mut ListNode) };
            new_node.size = merged_end - merged_start;
            new_node.next = next_node;
            current.next = Some(new_node);
        }

        self.total_allocated_bytes = self
            .total_allocated_bytes
            .checked_sub(alloc_size)
            .expect("total_allocated_bytes underflow");
        self.total_free_bytes = self
            .total_free_bytes
            .checked_add(alloc_size)
            .expect("total_free_bytes overflow");
        self.dealloc_count = self.dealloc_count.saturating_add(1);
    }
}

/// An interrupt-safe wrapper around `LinkedListAllocator` implementing `GlobalAlloc`.
pub struct LockedHeap {
    inner: Spinlock<LinkedListAllocator>,
}

impl LockedHeap {
    /// Creates a new uninitialized `LockedHeap`.
    pub const fn new() -> Self {
        LockedHeap {
            inner: Spinlock::new(LinkedListAllocator::new()),
        }
    }

    /// Locks the heap and returns current statistics.
    pub fn stats(&self) -> HeapStats {
        self.inner.lock().stats()
    }

    /// Initializes the underlying heap.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the given memory range is mapped and writable.
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) {
        self.inner.lock().init(heap_start, heap_size);
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner.lock().allocate_first_fit(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.lock().deallocate(ptr, layout)
    }
}

/// The global kernel heap allocator instance for JainOS.
#[global_allocator]
pub static ALLOCATOR: LockedHeap = LockedHeap::new();

/// Error types for kernel heap mapping.
#[derive(Debug)]
pub enum HeapMapError {
    HeapNotPageAligned,
    HeapSizeZeroOrNotPageAligned,
    AddressAlreadyMapped,
    FrameAllocationFailed { mapped_pages_count: usize },
    PageTableMappingError { mapped_pages_count: usize },
    CleanupUnmapError { unmapped_pages_count: usize, total_mapped: usize },
}

/// Helper function to unmap pages during partial mapping failure cleanup.
///
/// Flushes the TLB for each unmapped page.
///
/// # Note on Frame Reclamation
/// Physical frames returned from `mapper.unmap()` cannot be recycled
/// because `BootInfoFrameAllocator` is a monotonic cursor allocator without
/// frame deallocation support. The unmapping restores page-table integrity
/// and invalidates virtual translations.
unsafe fn cleanup_mapped_pages(
    mapper: &mut OffsetPageTable,
    page_range: x86_64::structures::paging::page::PageRangeInclusive<x86_64::structures::paging::Size4KiB>,
    count: usize,
) -> Result<(), HeapMapError> {
    let mut unmapped = 0;
    for (i, page) in page_range.enumerate() {
        if i >= count {
            break;
        }
        match mapper.unmap(page) {
            Ok((_frame, flush)) => {
                flush.flush();
                unmapped += 1;
            }
            Err(_) => {
                return Err(HeapMapError::CleanupUnmapError {
                    unmapped_pages_count: unmapped,
                    total_mapped: count,
                });
            }
        }
    }
    Ok(())
}

/// Maps the kernel heap virtual memory pages and initializes `ALLOCATOR`.
///
/// Ensures:
/// 1. Verifies that `HEAP_START` and `HEAP_SIZE` are 4 KiB page-aligned.
/// 2. Verifies that candidate heap range `[HEAP_START, HEAP_START + HEAP_SIZE)` is unmapped.
/// 3. Allocates genuine physical frames using `frame_allocator`.
/// 4. Maps pages with `PRESENT | WRITABLE | NO_EXECUTE` permissions in the active page table.
/// 5. Flushes the TLB for each mapped page.
/// 6. If any step fails, rolls back previously mapped pages before returning an error.
/// 7. Initializes `ALLOCATOR` only upon complete success.
pub fn init_heap(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut BootInfoFrameAllocator,
) -> Result<usize, HeapMapError> {
    if HEAP_START % 4096 != 0 {
        return Err(HeapMapError::HeapNotPageAligned);
    }
    if HEAP_SIZE == 0 || HEAP_SIZE % 4096 != 0 {
        return Err(HeapMapError::HeapSizeZeroOrNotPageAligned);
    }

    let heap_start = VirtAddr::new(HEAP_START as u64);
    let page_range = {
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // 1. Verify that no page in the proposed range is already mapped
    for page in page_range {
        if mapper.translate_page(page).is_ok() {
            return Err(HeapMapError::AddressAlreadyMapped);
        }
    }

    // 2. Map all heap pages with PRESENT | WRITABLE | NO_EXECUTE
    let mut mapped_count = 0;
    for page in page_range {
        let frame = match frame_allocator.allocate_frame() {
            Some(f) => f,
            None => {
                unsafe {
                    cleanup_mapped_pages(mapper, page_range, mapped_count)?;
                }
                return Err(HeapMapError::FrameAllocationFailed {
                    mapped_pages_count: mapped_count,
                });
            }
        };

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
        unsafe {
            match mapper.map_to(page, frame, flags, frame_allocator) {
                Ok(flush) => {
                    flush.flush();
                    mapped_count += 1;
                }
                Err(_) => {
                    cleanup_mapped_pages(mapper, page_range, mapped_count)?;
                    return Err(HeapMapError::PageTableMappingError {
                        mapped_pages_count: mapped_count,
                    });
                }
            }
        }
    }

    // 3. Initialize the global heap allocator only after all pages are mapped
    unsafe {
        ALLOCATOR.init(HEAP_START, HEAP_SIZE);
    }

    Ok(HEAP_SIZE)
}

/// Comprehensive test suite verifying heap allocation, deallocation,
/// alignment, prefix splitting, free-list traversal, forward/reverse coalescing,
/// non-adjacent isolation, large alignment, dynamic Rust types, repeated cycles,
/// and failure handling.
pub fn test_heap_allocations() {
    use alloc::boxed::Box;
    use alloc::string::String;
    use alloc::vec::Vec;

    // 1. Test single allocation and deallocation (Box)
    {
        let heap_value = Box::new(0xDEAD_BEEF_CAFE_BABE_u64);
        assert_eq!(*heap_value, 0xDEAD_BEEF_CAFE_BABE_u64);
    } // Box dropped and freed here

    // 2. Test multiple allocations with unique, non-overlapping addresses
    {
        let box1 = Box::new(100_u32);
        let box2 = Box::new(200_u32);
        let box3 = Box::new(300_u32);

        let p1 = &*box1 as *const u32 as usize;
        let p2 = &*box2 as *const u32 as usize;
        let p3 = &*box3 as *const u32 as usize;

        assert_ne!(p1, p2);
        assert_ne!(p2, p3);
        assert_ne!(p1, p3);

        assert_eq!(*box1, 100);
        assert_eq!(*box2, 200);
        assert_eq!(*box3, 300);
    }

    // 3. Test zero-size allocations (Requirement 7.C)
    {
        let zero_layout = Layout::from_size_align(0, 8).unwrap();
        unsafe {
            let p0 = ALLOCATOR.alloc(zero_layout);
            assert!(!p0.is_null(), "Zero-size allocation returned null");
            assert_eq!(p0 as usize % 8, 0, "Zero-size allocation not aligned");
            ALLOCATOR.dealloc(p0, zero_layout);
        }
    }

    // 4. Test diverse alignments (8, 16, 64, 128 bytes)
    {
        let l8 = Layout::from_size_align(32, 8).unwrap();
        let l16 = Layout::from_size_align(32, 16).unwrap();
        let l64 = Layout::from_size_align(64, 64).unwrap();
        let l128 = Layout::from_size_align(128, 128).unwrap();

        unsafe {
            let p8 = ALLOCATOR.alloc(l8);
            let p16 = ALLOCATOR.alloc(l16);
            let p64 = ALLOCATOR.alloc(l64);
            let p128 = ALLOCATOR.alloc(l128);

            assert!(!p8.is_null());
            assert!(!p16.is_null());
            assert!(!p64.is_null());
            assert!(!p128.is_null());

            assert_eq!(p8 as usize % 8, 0, "Alignment 8 failed");
            assert_eq!(p16 as usize % 16, 0, "Alignment 16 failed");
            assert_eq!(p64 as usize % 64, 0, "Alignment 64 failed");
            assert_eq!(p128 as usize % 128, 0, "Alignment 128 failed");

            ALLOCATOR.dealloc(p8, l8);
            ALLOCATOR.dealloc(p16, l16);
            ALLOCATOR.dealloc(p64, l64);
            ALLOCATOR.dealloc(p128, l128);
        }
    }

    // 5. Test prefix splitting: force non-zero prefix gap into free list (Requirement 7.A)
    {
        // Allocate a small block to offset the start of the free list
        let l_offset = Layout::from_size_align(32, 8).unwrap();
        // Request alignment 64: free list starts at (offset 32), which is not 64-aligned.
        // Needs prefix gap = 32 bytes (which is >= 16 bytes for ListNode), creating a prefix free node!
        let l_align64 = Layout::from_size_align(64, 64).unwrap();

        unsafe {
            let p_offset = ALLOCATOR.alloc(l_offset);
            assert!(!p_offset.is_null());

            let p_aligned = ALLOCATOR.alloc(l_align64);
            assert!(!p_aligned.is_null());
            assert_eq!(p_aligned as usize % 64, 0, "Alignment 64 failed in prefix test");
            // Verify there is a prefix gap between p_offset + 32 and p_aligned
            assert!(
                p_aligned as usize >= p_offset as usize + 32 + core::mem::size_of::<ListNode>(),
                "Prefix gap was not created"
            );

            // Now allocate into the prefix gap: should allocate at p_offset + 32
            let l_prefix_fill = Layout::from_size_align(16, 8).unwrap();
            let p_prefix = ALLOCATOR.alloc(l_prefix_fill);
            assert!(!p_prefix.is_null());
            assert_eq!(
                p_prefix as usize,
                p_offset as usize + 32,
                "Allocation did not carve out of the split prefix node"
            );

            ALLOCATOR.dealloc(p_prefix, l_prefix_fill);
            ALLOCATOR.dealloc(p_aligned, l_align64);
            ALLOCATOR.dealloc(p_offset, l_offset);
        }
    }

    // 6. Test free-list traversal: skip smaller first free block for larger later block (Requirement 7.B)
    {
        let l_small = Layout::from_size_align(32, 8).unwrap();
        let l_sep = Layout::from_size_align(32, 8).unwrap();
        let l_large = Layout::from_size_align(512, 8).unwrap();

        unsafe {
            let b1 = ALLOCATOR.alloc(l_small);
            let b2 = ALLOCATOR.alloc(l_sep); // Separator to prevent coalescing
            let b3 = ALLOCATOR.alloc(l_large);

            assert!(!b1.is_null() && !b2.is_null() && !b3.is_null());

            // Free b1 (creates 32-byte block) and b3 (creates 512-byte block)
            ALLOCATOR.dealloc(b1, l_small);
            ALLOCATOR.dealloc(b3, l_large);

            // Request 256 bytes: cannot fit in b1 (32 B), must traverse past b1 to b3!
            let l_req = Layout::from_size_align(256, 8).unwrap();
            let b_new = ALLOCATOR.alloc(l_req);
            assert!(!b_new.is_null(), "Traversal allocation failed");
            assert_eq!(b_new as usize, b3 as usize, "Allocator did not skip b1 to allocate from b3");

            ALLOCATOR.dealloc(b_new, l_req);
            ALLOCATOR.dealloc(b2, l_sep);
        }
    }

    // 7. Test forward coalescing (free in A, B, C order) (Requirement 7.D)
    {
        let l = Layout::from_size_align(128, 16).unwrap();
        unsafe {
            let a = ALLOCATOR.alloc(l);
            let b = ALLOCATOR.alloc(l);
            let c = ALLOCATOR.alloc(l);

            assert!(!a.is_null() && !b.is_null() && !c.is_null());

            // Free in forward order: A, then B, then C
            ALLOCATOR.dealloc(a, l);
            ALLOCATOR.dealloc(b, l);
            ALLOCATOR.dealloc(c, l);

            // Reallocate unified block spanning all three (384 bytes)
            let combined_layout = Layout::from_size_align(384, 16).unwrap();
            let unified = ALLOCATOR.alloc(combined_layout);
            assert!(!unified.is_null(), "Forward coalescing reallocation failed");
            assert_eq!(unified, a, "Unified block did not start at A's original address");

            ALLOCATOR.dealloc(unified, combined_layout);
        }
    }

    // 8. Test reverse coalescing (free in B, A, C order)
    {
        let l = Layout::from_size_align(128, 16).unwrap();
        unsafe {
            let a = ALLOCATOR.alloc(l);
            let b = ALLOCATOR.alloc(l);
            let c = ALLOCATOR.alloc(l);

            assert!(!a.is_null() && !b.is_null() && !c.is_null());

            ALLOCATOR.dealloc(b, l);
            ALLOCATOR.dealloc(a, l);
            ALLOCATOR.dealloc(c, l);

            let combined_layout = Layout::from_size_align(384, 16).unwrap();
            let unified = ALLOCATOR.alloc(combined_layout);
            assert!(!unified.is_null(), "Reverse coalescing reallocation failed");
            assert_eq!(unified, a, "Unified block did not start at A's original base");

            ALLOCATOR.dealloc(unified, combined_layout);
        }
    }

    // 9. Test non-adjacent free blocks are not incorrectly merged (Requirement 7.E)
    {
        let l = Layout::from_size_align(128, 16).unwrap();
        unsafe {
            let a = ALLOCATOR.alloc(l);
            let b = ALLOCATOR.alloc(l); // Separator between A and C
            let c = ALLOCATOR.alloc(l);
            let d = ALLOCATOR.alloc(l); // Separator between C and rest of heap

            assert!(!a.is_null() && !b.is_null() && !c.is_null() && !d.is_null());

            // Free non-adjacent blocks A and C, leaving B and D allocated
            ALLOCATOR.dealloc(a, l);
            ALLOCATOR.dealloc(c, l);

            // Attempting to allocate 200 bytes must NOT merge A and C across allocated B.
            // Since A is 128 B and C is 128 B, neither can satisfy 200 B.
            // Therefore, the allocation must skip both A and C and be allocated after D.
            let l_200 = Layout::from_size_align(200, 16).unwrap();
            let p_200 = ALLOCATOR.alloc(l_200);
            assert!(!p_200.is_null());
            assert_ne!(p_200, a, "A was incorrectly expanded or merged");
            assert_ne!(p_200, c, "C was incorrectly expanded or merged");
            assert!(
                p_200 as usize >= d as usize + 128,
                "Non-adjacent free blocks were incorrectly merged across allocated block B"
            );

            ALLOCATOR.dealloc(p_200, l_200);
            ALLOCATOR.dealloc(b, l);
            ALLOCATOR.dealloc(d, l);
        }
    }

    // 10. Test larger alignments: 256, 512, and 4096 bytes (Requirement 7.G)
    {
        let l256 = Layout::from_size_align(64, 256).unwrap();
        let l512 = Layout::from_size_align(64, 512).unwrap();
        let l4096 = Layout::from_size_align(64, 4096).unwrap();

        unsafe {
            let p256 = ALLOCATOR.alloc(l256);
            let p512 = ALLOCATOR.alloc(l512);
            let p4096 = ALLOCATOR.alloc(l4096);

            assert!(!p256.is_null(), "256-byte alignment allocation failed");
            assert!(!p512.is_null(), "512-byte alignment allocation failed");
            assert!(!p4096.is_null(), "4096-byte alignment allocation failed");

            assert_eq!(p256 as usize % 256, 0, "Alignment to 256 bytes failed");
            assert_eq!(p512 as usize % 512, 0, "Alignment to 512 bytes failed");
            assert_eq!(p4096 as usize % 4096, 0, "Alignment to 4096 bytes failed");

            ALLOCATOR.dealloc(p256, l256);
            ALLOCATOR.dealloc(p512, l512);
            ALLOCATOR.dealloc(p4096, l4096);
        }
    }

    // 11. Test dynamic vector allocation and geometric growth
    {
        let mut vec = Vec::new();
        for i in 0..500 {
            vec.push(i);
        }
        assert_eq!(vec.len(), 500);
        for i in 0..500 {
            assert_eq!(vec[i], i);
        }
    }

    // 12. Test dynamic string allocation and formatting
    {
        let mut s = String::from("JainOS");
        s.push_str(" Educational Kernel");
        assert_eq!(s.as_str(), "JainOS Educational Kernel");
    }

    // 13. Test repeated allocation and deallocation cycles (Requirement 7.F)
    {
        for cycle in 0..100 {
            let mut v = Vec::with_capacity(10);
            for i in 0..10 {
                v.push(cycle * 10 + i);
            }
            assert_eq!(v.len(), 10);
            let b = Box::new(cycle as u64);
            assert_eq!(*b, cycle as u64);
        }
    }

    // 14. Test allocation failure handling (requesting size exceeding entire heap)
    {
        let impossible_layout = Layout::from_size_align(32 * 1024 * 1024, 8).unwrap();
        let initial_failures = ALLOCATOR.stats().alloc_failures;

        unsafe {
            let ptr = ALLOCATOR.alloc(impossible_layout);
            assert!(ptr.is_null(), "Impossible allocation should return null");
        }

        assert_eq!(
            ALLOCATOR.stats().alloc_failures,
            initial_failures + 1,
            "Allocation failure count not incremented"
        );

        let recovery_box = Box::new(42_u64);
        assert_eq!(*recovery_box, 42);
    }
}
