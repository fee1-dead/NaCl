use alloc::boxed::Box;
use alloc::vec;
use core::{fmt, slice};

use hashbrown::HashMap;
use stivale_boot::v2::StivaleFramebufferTag;
use x86_64::instructions::interrupts::without_interrupts;

use crate::task::lock::Mutex;

static FT: &[u8] = include_bytes!("ter-u20n.psf");

static FBMAN: Mutex<Option<FrameBufferManager>> = Mutex::new(None);

pub(crate) fn insert_fbman(fbman: FrameBufferManager) {
    let mut guard = FBMAN.lock_or_spin();
    debug_assert!(guard.is_none());

    *guard = Some(fbman);
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;

    // avoid deadlocks by disabling interrupts before aquiring the lock,
    // enabling interrupts after lock is released.
    without_interrupts(|| {
        FBMAN
            .lock_or_spin()
            .as_mut()
            .expect("screen uninitialized")
            .write_fmt(args)
            .expect("Printing to screen failed");
    });
}

/// Prints to the screen
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::font::_print(format_args!($($arg)*));
    };
}

/// Prints to the screen.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::font::_print(format_args_nl!($fmt)));
    ($fmt:expr, $($arg:tt)*) => ($crate::font::_print(
        format_args_nl!($fmt, $($arg)*)
    ));
}

pub struct PsfError;

#[repr(C)]
#[derive(Debug)]
pub struct PsfHeader {
    magic: u32,
    version: u32,
    headersize: u32,
    flags: u32,
    glyphs: u32,
    bytes_per_glyph: u32,
    height: u32,
    width: u32,
}

impl PsfHeader {
    pub fn ft() -> &'static Self {
        unsafe { FT.as_ptr().cast::<Self>().as_ref().unwrap() }
    }

    fn unicode_mapping(&self) -> Option<HashMap<char, u32>> {
        if self.flags == 0 {
            return None;
        }

        let mut s = (self.headersize + self.glyphs * self.bytes_per_glyph) as usize;
        let mut start = s;
        let mut glyph = 0;
        let mut map = HashMap::new();
        while s < FT.len() {
            if FT[s] == 0xFF {
                let string = core::str::from_utf8(&FT[start..s]).expect("valid utf-8");
                for ch in string.chars() {
                    map.insert(ch, glyph);
                }

                glyph += 1;
                s += 1;
                start = s;
                continue;
            }

            s += 1;
        }

        Some(map)
    }
}

pub struct FrameBufferManager {
    fb: &'static mut [u8],
    mapping: Option<HashMap<char, u32>>,
    pub chars: Box<[char]>,
    pub horiz_chars: usize,
    pub bytes_per_pixel: usize,
    pub stride: usize,
    idx: usize,
    /// Shift of the red mask in RGB.
    pub red_mask_shift: u8,
    /// Shift of the green mask in RGB.
    pub green_mask_shift: u8,
    /// Shift of the blue mask in RGB.
    pub blue_mask_shift: u8,
}

impl fmt::Debug for FrameBufferManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrameBufferManager")
            .field("horiz_chars", &self.horiz_chars)
            .field("bytes_per_pixel", &self.bytes_per_pixel)
            .field("stride", &self.stride)
            .field("idx", &self.idx)
            .field("red_mask_shift", &self.red_mask_shift)
            .field("green_mask_shift", &self.green_mask_shift)
            .field("blue_mask_shift", &self.blue_mask_shift)
            .finish()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: u32,
    pub g: u32,
    pub b: u32,
}

impl FrameBufferManager {
    pub fn new(tag: &'static StivaleFramebufferTag) -> Self {
        let font = PsfHeader::ft();

        let mapping = font.unicode_mapping();

        let horiz_res = tag.framebuffer_width as usize;
        let horiz_chars = horiz_res / font.width as usize;

        let vert_res = tag.framebuffer_height as usize;
        let vert_chars = vert_res / font.height as usize;

        let chars = vec![' '; horiz_chars * vert_chars].into_boxed_slice();

        let bytes_per_pixel = (tag.framebuffer_bpp / 8) as usize;
        let stride = tag.framebuffer_pitch as usize;

        let fb = unsafe { slice::from_raw_parts_mut(tag.framebuffer_addr as *mut u8, tag.size()) };

        Self {
            fb,
            mapping,
            chars,
            horiz_chars,
            bytes_per_pixel,
            stride,
            idx: 0,
            red_mask_shift: tag.red_mask_shift,
            green_mask_shift: tag.green_mask_shift,
            blue_mask_shift: tag.blue_mask_shift,
        }
    }

    pub fn put(&mut self, c: char) {
        if c == '\n' {
            self.idx = 0;
            self.newline();
            self.redraw();
            return;
        }

        let last_line = self.chars.len() - self.horiz_chars;

        if self.idx == self.horiz_chars {
            // content wraps to the next line
            self.idx = 0;
            self.newline();
            let offset = last_line;
            self.chars[offset] = c;
            self.redraw()
        } else {
            let offset = last_line + self.idx;
            self.chars[offset] = c;
            self.putchar(
                c,
                self.idx,
                self.chars.len() / self.horiz_chars - 1,
                0xFFFFFF,
                0,
            );
        }

        self.idx += 1;
    }

    /// Redraw the whole grid.
    fn redraw(&mut self) {
        let mut x = 0;
        let mut y = 0;
        let horiz_chars = self.horiz_chars;
        for &c in self.chars.as_ref() {
            Self::putc(
                self.fb,
                self.bytes_per_pixel,
                self.stride,
                &self.mapping,
                c,
                x,
                y,
                u32::MAX,
                0,
            );
            if x + 1 == horiz_chars {
                y += 1;
                x = 0;
            } else {
                x += 1;
            }
        }
    }

    #[inline]
    fn newline(&mut self) {
        self.chars.rotate_left(self.horiz_chars);
        let len = self.chars.len();
        self.chars[len - self.horiz_chars..].fill(' ');
    }

    fn putc(
        fb: &mut [u8],
        bytes_per_pixel: usize,
        stride: usize,
        mapping: &Option<HashMap<char, u32>>,
        c: char,
        cx: usize,
        cy: usize,
        fg: u32,
        bg: u32,
    ) {
        assert_eq!(4, bytes_per_pixel);

        let font = PsfHeader::ft();
        let font_height = font.height as usize;
        let font_width = font.width as usize;
        let bytes_per_line = (font_width + 7) / 8;
        let c = mapping
            .as_ref()
            .map_or(c as u32, |m| m.get(&c).copied().unwrap_or(0));

        // If there is no glyph for the character, we will display the first glyph.
        let glyph_index = if c >= font.glyphs { 0 } else { c as usize };

        let mut glyph = unsafe {
            FT.as_ptr()
                .add(font.headersize as usize)
                .add(glyph_index * font.bytes_per_glyph as usize)
        };

        let mut offset = (cy * font_height * stride) + (cx * font_width * bytes_per_pixel);

        for _ in 0..font_height {
            let mut line = offset;
            let mut mask = 1 << (font_width - 1);

            let gly = unsafe { (*glyph.cast::<u16>()).rotate_left(1) };

            for _ in 0..font_width {
                unsafe {
                    let pixel = fb.as_mut_ptr().add(line) as *mut u32;
                    pixel.write_volatile(if gly & mask != 0 { fg } else { bg });
                }
                mask >>= 1;
                line += bytes_per_pixel;
            }
            unsafe {
                glyph = glyph.add(bytes_per_line);
            }
            offset += stride;
        }
    }

    pub fn putchar(&mut self, c: char, cx: usize, cy: usize, fg: u32, bg: u32) {
        Self::putc(
            self.fb,
            self.bytes_per_pixel,
            self.stride,
            &self.mapping,
            c,
            cx,
            cy,
            fg,
            bg,
        )
    }
}

impl core::fmt::Write for FrameBufferManager {
    fn write_char(&mut self, c: char) -> core::fmt::Result {
        self.put(c);
        Ok(())
    }
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.put(c)
        }
        Ok(())
    }
}
