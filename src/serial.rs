use core::lazy::Lazy;
use core::sync::atomic::{AtomicBool, Ordering};

use uart_16550::SerialPort;

use crate::task::lock::{Mutex, MutexGuard};

static SERIAL1: Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(0x3F8) });
static INIT: AtomicBool = AtomicBool::new(false);

fn serial1() -> MutexGuard<'static, SerialPort> {
    if !INIT.swap(true, core::sync::atomic::Ordering::Relaxed) {
        let mut guard = SERIAL1.try_lock().unwrap();
        guard.init();
        guard
    } else {
        SERIAL1.try_lock().unwrap()
    }
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    serial1()
        .write_fmt(args)
        .expect("Printing to serial failed");
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! sprint {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! sprintln {
    () => ($crate::sprint!("\n"));
    ($fmt:expr) => ($crate::sprint!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::sprint!(
        concat!($fmt, "\n"), $($arg)*));
}
