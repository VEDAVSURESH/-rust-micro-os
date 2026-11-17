#![no_std]
#![no_main]

use core::panic::PanicInfo;

static HELLO: &[u8] = b"Hello from Rust Micro-OS!";

/// Entry point of the kernel, called by the bootloader after setting up
/// 64-bit long mode and virtual memory paging.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    let vga_buffer = 0xb8000 as *mut u8;

    // Directly write characters to the VGA text buffer memory at 0xb8000
    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0x0b; // Light cyan on black background
        }
    }

    loop {}
}

/// This function is called on panic (e.g., assertion failure or array bounds check).
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
