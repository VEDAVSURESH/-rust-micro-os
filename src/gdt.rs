//! Global Descriptor Table (GDT) and Task State Segment (TSS).
//!
//! On x86_64, segmentation is largely obsolete for memory isolation (paging is used instead).
//! However, the GDT is still mandatory for two critical reasons:
//! 1. Switching between kernel space (Ring 0) and user space (Ring 3).
//! 2. Loading a Task State Segment (TSS) that holds the Interrupt Stack Table (IST).
//!
//! The IST allows exceptions (like Double Fault) to execute on a dedicated, known-good
//! kernel stack, preventing triple faults when the primary stack overflows.

use core::cell::UnsafeCell;
use x86_64::VirtAddr;
use x86_64::instructions::segmentation::{CS, Segment};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

/// Index in the TSS Interrupt Stack Table reserved for Double Fault exceptions.
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

/// Dedicated stack size for the Double Fault exception handler (20 KiB).
const STACK_SIZE: usize = 4096 * 5;

/// Static memory buffer used as the dedicated Double Fault stack.
static mut DOUBLE_FAULT_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

/// Holds the TSS with interior mutability.
struct TssStorage(UnsafeCell<TaskStateSegment>);
unsafe impl Sync for TssStorage {}

static TSS: TssStorage = TssStorage(UnsafeCell::new(TaskStateSegment::new()));

/// Holds GDT and segment selectors permanently in memory.
struct GdtStorage {
    gdt: GlobalDescriptorTable,
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

struct GdtWrapper(UnsafeCell<Option<GdtStorage>>);
unsafe impl Sync for GdtWrapper {}

static GDT: GdtWrapper = GdtWrapper(UnsafeCell::new(None));

/// Initializes the GDT and TSS, loading them into the CPU registers.
pub fn init() {
    unsafe {
        // Set up the dedicated stack in the TSS Interrupt Stack Table (IST).
        // x86 stacks grow downwards (from high address to low address), so top of stack is start + STACK_SIZE.
        let stack_start = VirtAddr::from_ptr(&raw const DOUBLE_FAULT_STACK);
        let stack_end = stack_start + STACK_SIZE;
        let tss = &mut *TSS.0.get();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = stack_end;

        // Populate the Global Descriptor Table
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&*TSS.0.get()));

        let gdt_ptr = &mut *GDT.0.get();
        *gdt_ptr = Some(GdtStorage {
            gdt,
            code_selector,
            tss_selector,
        });

        let storage = gdt_ptr.as_ref().unwrap();

        // Load GDT into CPU's GDTR register
        storage.gdt.load();

        // Reload Code Segment (CS) register with the new kernel code descriptor
        CS::set_reg(storage.code_selector);

        // Load Task Register (TR) with the TSS descriptor
        load_tss(storage.tss_selector);
    }
}
