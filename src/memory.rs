//! Physical memory management, safe frame boundary calculations, and frame allocation for JainOS.
//!
//! This module provides:
//! - Conservative general safe-frame calculation excluding boundary frames that could be partial
//!   due to bootloader E820 conversion rounding.
//! - `BootInfoFrameAllocator` safely dispensing 4 KiB physical frames from the bootloader memory map.
//! - Checked arithmetic for all memory statistics and frame indexing with documented overflow handling.

use bootloader::bootinfo::{MemoryMap, MemoryRegion, MemoryRegionType};
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

/// Initializes an `OffsetPageTable` pointing to the active Level 4 Page Table.
///
/// # Safety
///
/// The caller must guarantee that the complete physical memory is mapped to
/// virtual memory at the passed `physical_memory_offset`.
/// Also, this function must be called only once to avoid aliasing mutable references
/// to the active page table.
pub unsafe fn init_offset_page_table(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    let level_4_table: &'static mut PageTable = &mut *page_table_ptr;
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Physical memory statistics derived strictly from safe, non-partial usable memory frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalMemoryInfo {
    /// Total number of memory regions reported by BIOS / bootloader.
    pub total_regions: usize,
    /// Number of regions containing safely allocatable Usable RAM.
    pub usable_regions: usize,
    /// Total amount of genuinely safe physical RAM in bytes.
    pub total_usable_bytes: u64,
    /// Total count of completely enclosed 4 KiB frames across all usable regions.
    pub total_usable_frames: u64,
}

/// Errors that can occur during memory map analysis or frame allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryAnalysisError {
    /// Arithmetic overflow occurred while aggregating frame or byte totals.
    ArithmeticOverflow,
}

/// Computes the safe, non-partial, allocatable 4 KiB physical frame range `[start, end)`
/// for a given memory region.
///
/// # Mathematical Safety Guarantee
///
/// In `bootloader 0.9.35`, BIOS E820 raw byte ranges `[start_addr, end_addr)` are converted into
/// `FrameRange { start_frame_number, end_frame_number }` via integer division:
/// - `start_frame_number = start_addr / 4096` (rounds DOWN)
/// - `end_frame_number = ((end_addr - 1) / 4096) + 1` (rounds UP)
///
/// Because the bootloader only stores frame numbers and discards original byte offsets:
/// 1. If `start_addr` was unaligned (`start_addr % 4096 != 0`), `start_frame_number` starts at
///    `start_frame_number * 4096 < start_addr`. The initial frame is therefore a partial frame,
///    containing bytes prior to `start_addr` that were not marked Usable by the BIOS.
/// 2. If `end_addr` was unaligned (`end_addr % 4096 != 0`), `end_frame_number - 1` ends at
///    `end_frame_number * 4096 > end_addr`. The final frame is therefore a partial frame,
///    containing bytes after `end_addr` that were not marked Usable by the BIOS.
///
/// To guarantee with absolute mathematical certainty that every allocated frame is 100%
/// contained within genuinely usable physical RAM:
/// - `safe_start = raw_start + 1`: Advances past the potentially partial initial frame.
///   Since `start_addr < (raw_start + 1) * 4096`, frame `raw_start + 1` is guaranteed to contain
///   zero bytes prior to `start_addr`. Also, if `raw_start == 0`, `safe_start` becomes `1`,
///   naturally and unconditionally excluding physical frame zero (`0x0000_0000`).
/// - `safe_end = raw_end - 1`: Trims the potentially partial final frame.
///   Since `(raw_end - 1) * 4096 <= end_addr`, all frames prior to `safe_end` end at or before
///   `end_addr`, guaranteed to contain zero bytes past `end_addr`.
///
/// Any region with fewer than 3 raw frames cannot guarantee a complete, non-partial frame
/// and yields `None`.
pub fn safe_usable_frame_range(region: &MemoryRegion) -> Option<(u64, u64)> {
    if region.region_type != MemoryRegionType::Usable {
        return None;
    }

    let raw_start = region.range.start_frame_number;
    let raw_end = region.range.end_frame_number;

    // Conservative boundary trimming using checked arithmetic:
    // Exclude the first frame (raw_start) and the last frame (raw_end - 1)
    let candidate_start = raw_start.checked_add(1)?;
    let safe_start = core::cmp::max(candidate_start, 1);
    let safe_end = raw_end.checked_sub(1)?;

    if safe_start < safe_end {
        Some((safe_start, safe_end))
    } else {
        None
    }
}

/// Computes the safe 4 KiB physical frame range `[start, end)` from raw physical byte addresses.
///
/// Ensures:
/// - Start address is rounded UP (ceiling) to the next 4 KiB boundary, ensuring no bytes
///   before `start_addr` are included in the first frame.
/// - End address is rounded DOWN (floor) to the previous 4 KiB boundary, ensuring no bytes
///   after `end_addr` are included in the last frame.
/// - Frame zero is excluded (`start >= 1`).
/// - If the usable range is smaller than a single 4 KiB frame, returns `None`.
pub fn safe_frame_range_from_addrs(start_addr: u64, end_addr: u64) -> Option<(u64, u64)> {
    if start_addr >= end_addr {
        return None;
    }

    // Round start address UP to next 4 KiB boundary (ceiling division)
    let start_aligned = start_addr.checked_add(4095)? / 4096;

    // Round end address DOWN to previous 4 KiB boundary (floor division)
    let end_aligned = end_addr / 4096;

    // Exclude physical frame zero
    let safe_start = core::cmp::max(start_aligned, 1);

    if safe_start < end_aligned {
        Some((safe_start, end_aligned))
    } else {
        None
    }
}

/// Analyzes the bootloader memory map to compute safe physical memory statistics.
///
/// Uses explicit checked arithmetic throughout. If an arithmetic overflow occurs
/// (e.g. invalid or corrupt descriptors), this function returns an error instead of
/// silently saturating or truncating, ensuring that inaccurate estimates are never
/// reported as exact totals.
pub fn analyze_memory_map(memory_map: &MemoryMap) -> Result<PhysicalMemoryInfo, MemoryAnalysisError> {
    let mut total_regions: usize = 0;
    let mut usable_regions: usize = 0;
    let mut total_usable_frames: u64 = 0;

    for region in memory_map.iter() {
        total_regions = total_regions
            .checked_add(1)
            .ok_or(MemoryAnalysisError::ArithmeticOverflow)?;

        if let Some((start, end)) = safe_usable_frame_range(region) {
            usable_regions = usable_regions
                .checked_add(1)
                .ok_or(MemoryAnalysisError::ArithmeticOverflow)?;

            let frames_in_region = end
                .checked_sub(start)
                .ok_or(MemoryAnalysisError::ArithmeticOverflow)?;

            total_usable_frames = total_usable_frames
                .checked_add(frames_in_region)
                .ok_or(MemoryAnalysisError::ArithmeticOverflow)?;
        }
    }

    let total_usable_bytes = total_usable_frames
        .checked_mul(4096)
        .ok_or(MemoryAnalysisError::ArithmeticOverflow)?;

    Ok(PhysicalMemoryInfo {
        total_regions,
        usable_regions,
        total_usable_bytes,
        total_usable_frames,
    })
}

/// Verifies whether a given physical frame lies completely within a safely allocatable usable region.
///
/// Uses `safe_usable_frame_range` to ensure that boundary partial frames are considered non-usable.
pub fn is_frame_in_usable_region(memory_map: &MemoryMap, frame: PhysFrame<Size4KiB>) -> bool {
    let frame_num = frame.start_address().as_u64() / 4096;
    if frame_num == 0 {
        return false; // Physical frame 0 is never considered usable
    }

    for region in memory_map.iter() {
        if let Some((start, end)) = safe_usable_frame_range(region) {
            if frame_num >= start && frame_num < end {
                return true;
            }
        }
    }

    false
}

/// A physical frame allocator that uses the bootloader's memory map.
///
/// Allocations are performed in O(1) amortized time using a monotonic region-and-frame cursor.
/// Every allocated frame is strictly validated against `safe_usable_frame_range`.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    current_region_idx: usize,
    current_frame: u64,
}

impl BootInfoFrameAllocator {
    /// Initializes a `BootInfoFrameAllocator` from the bootloader's memory map.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that:
    /// 1. The passed `memory_map` is valid and was provided by the bootloader.
    /// 2. Frames marked as `Usable` in the memory map are genuinely unused and will not
    ///    be allocated by any other allocator simultaneously.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        let mut allocator = BootInfoFrameAllocator {
            memory_map,
            current_region_idx: 0,
            current_frame: 0,
        };
        allocator.advance_to_next_usable_region();
        allocator
    }

    /// Advances `current_region_idx` until a safe, usable, non-empty region is found,
    /// or until all regions are exhausted.
    fn advance_to_next_usable_region(&mut self) {
        while self.current_region_idx < self.memory_map.len() {
            let region = &self.memory_map[self.current_region_idx];
            if let Some((safe_start, _)) = safe_usable_frame_range(region) {
                self.current_frame = safe_start;
                return;
            }
            // Use checked addition; if index overflows, stop searching
            match self.current_region_idx.checked_add(1) {
                Some(next_idx) => self.current_region_idx = next_idx,
                None => {
                    self.current_region_idx = self.memory_map.len();
                    return;
                }
            }
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    /// Allocates the next available 4 KiB physical frame.
    ///
    /// Returns `Some(PhysFrame)` on success, or `None` if physical memory is exhausted.
    /// Operates in O(1) amortized time with checked arithmetic, strictly within `safe_usable_frame_range`.
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        while self.current_region_idx < self.memory_map.len() {
            let region = &self.memory_map[self.current_region_idx];

            if let Some((_safe_start, safe_end)) = safe_usable_frame_range(region) {
                if self.current_frame < safe_end {
                    let frame_num = self.current_frame;
                    // Safely compute physical address: frame_num * 4096
                    let phys_addr_val = frame_num.checked_mul(4096)?;
                    let frame = PhysFrame::containing_address(PhysAddr::new(phys_addr_val));

                    // Advance monotonic frame cursor using checked addition
                    self.current_frame = self.current_frame.checked_add(1)?;
                    return Some(frame);
                }
            }

            // Current region is exhausted or not usable; advance to next region
            self.current_region_idx = self.current_region_idx.checked_add(1)?;
            self.advance_to_next_usable_region();
        }

        None
    }
}

/// Comprehensive test suite verifying unaligned boundary safety,
/// partial frame exclusion, checked arithmetic, and safe frame range calculations.
pub fn test_unaligned_boundary_safety() {
    // Test Case 1: General conservative trimming on MemoryRegion
    // Region spanning frames 256..261 (5 frames: 256, 257, 258, 259, 260).
    // Conservative trimming excludes frame 256 (partial start) and frame 260 (partial end).
    // Expected safe range: [257, 260) -> frames 257, 258, 259.
    let region_5frames = bootloader::bootinfo::MemoryRegion {
        range: bootloader::bootinfo::FrameRange {
            start_frame_number: 256,
            end_frame_number: 261,
        },
        region_type: bootloader::bootinfo::MemoryRegionType::Usable,
    };
    assert_eq!(
        safe_usable_frame_range(&region_5frames),
        Some((257, 260)),
        "Test Case 1 Failed: general conservative trimming incorrect"
    );

    // Test Case 2: Region with only 1 frame (cannot guarantee complete frame)
    let region_1frame = bootloader::bootinfo::MemoryRegion {
        range: bootloader::bootinfo::FrameRange {
            start_frame_number: 100,
            end_frame_number: 101,
        },
        region_type: bootloader::bootinfo::MemoryRegionType::Usable,
    };
    assert_eq!(
        safe_usable_frame_range(&region_1frame),
        None,
        "Test Case 2 Failed: single-frame region must return None"
    );

    // Test Case 3: Region with only 2 frames (both may be partial boundary frames)
    let region_2frames = bootloader::bootinfo::MemoryRegion {
        range: bootloader::bootinfo::FrameRange {
            start_frame_number: 100,
            end_frame_number: 102,
        },
        region_type: bootloader::bootinfo::MemoryRegionType::Usable,
    };
    assert_eq!(
        safe_usable_frame_range(&region_2frames),
        None,
        "Test Case 3 Failed: two-frame region must return None"
    );

    // Test Case 4: Region with 3 frames (middle frame is guaranteed 100% enclosed)
    let region_3frames = bootloader::bootinfo::MemoryRegion {
        range: bootloader::bootinfo::FrameRange {
            start_frame_number: 100,
            end_frame_number: 103,
        },
        region_type: bootloader::bootinfo::MemoryRegionType::Usable,
    };
    assert_eq!(
        safe_usable_frame_range(&region_3frames),
        Some((101, 102)),
        "Test Case 4 Failed: three-frame region must yield middle frame"
    );

    // Test Case 5: Frame zero exclusion (raw start 0 must advance to at least 1)
    let region_framezero = bootloader::bootinfo::MemoryRegion {
        range: bootloader::bootinfo::FrameRange {
            start_frame_number: 0,
            end_frame_number: 10,
        },
        region_type: bootloader::bootinfo::MemoryRegionType::Usable,
    };
    assert_eq!(
        safe_usable_frame_range(&region_framezero),
        Some((1, 9)),
        "Test Case 5 Failed: frame zero region must yield [1, 9)"
    );

    // Test Case 6: Non-usable region must return None
    let reserved_region = bootloader::bootinfo::MemoryRegion {
        range: bootloader::bootinfo::FrameRange {
            start_frame_number: 100,
            end_frame_number: 200,
        },
        region_type: bootloader::bootinfo::MemoryRegionType::Reserved,
    };
    assert_eq!(
        safe_usable_frame_range(&reserved_region),
        None,
        "Test Case 6 Failed: non-usable region must return None"
    );

    // Test Case 7: Raw byte address ceiling/floor calculations
    let raw_unaligned = safe_frame_range_from_addrs(0x100_800, 0x103_800);
    assert_eq!(
        raw_unaligned,
        Some((257, 259)),
        "Test Case 7 Failed: raw byte unaligned range incorrect"
    );

    // Test Case 8: Sub-page raw byte range (< 4 KiB) returns None
    let raw_subpage = safe_frame_range_from_addrs(0x100_200, 0x100_e00);
    assert_eq!(
        raw_subpage,
        None,
        "Test Case 8 Failed: subpage raw range must return None"
    );
}
