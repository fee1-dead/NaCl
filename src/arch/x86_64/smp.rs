//! Simultanous multi processing.

use core::arch::asm;

/// initialize application processor.
///
/// There is a lot to be done here: the AP boots on real mode,
/// we need to set up the stack, make and load global descriptor table,
/// setting up and enabling paging, and switch to long mode to be able
/// to call Rust code. Fortunately we do not need to set up a framebuffer
/// and all that kinds of stuff.
#[naked]
pub unsafe extern "C" fn ap_init() {
    asm!("
        0:
            hlt
            jmp 0
    ", options(noreturn));
}