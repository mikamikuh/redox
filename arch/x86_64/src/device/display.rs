use core::{cmp, slice};
use ransid::Console;
use spin::Mutex;

use externs::memset;
use memory::Frame;
use paging::{ActivePageTable, PhysicalAddress, entry};

/// The info of the VBE mode
#[derive(Copy, Clone, Default, Debug)]
#[repr(packed)]
pub struct VBEModeInfo {
    attributes: u16,
    win_a: u8,
    win_b: u8,
    granularity: u16,
    winsize: u16,
    segment_a: u16,
    segment_b: u16,
    winfuncptr: u32,
    bytesperscanline: u16,
    pub xresolution: u16,
    pub yresolution: u16,
    xcharsize: u8,
    ycharsize: u8,
    numberofplanes: u8,
    bitsperpixel: u8,
    numberofbanks: u8,
    memorymodel: u8,
    banksize: u8,
    numberofimagepages: u8,
    unused: u8,
    redmasksize: u8,
    redfieldposition: u8,
    greenmasksize: u8,
    greenfieldposition: u8,
    bluemasksize: u8,
    bluefieldposition: u8,
    rsvdmasksize: u8,
    rsvdfieldposition: u8,
    directcolormodeinfo: u8,
    physbaseptr: u32,
    offscreenmemoryoffset: u32,
    offscreenmemsize: u16,
}

pub static DISPLAY: Mutex<Option<Display>> = Mutex::new(None);
static FONT: &'static [u8] = include_bytes!("../../../../res/unifont.font");

pub unsafe fn init(active_table: &mut ActivePageTable) {
    active_table.identity_map(Frame::containing_address(PhysicalAddress::new(0x5200)), entry::PRESENT | entry::NO_EXECUTE);

    let mode_info = &*(0x5200 as *const VBEModeInfo);
    if mode_info.physbaseptr > 0 {
        let width = mode_info.xresolution as usize;
        let height = mode_info.yresolution as usize;
        let start = mode_info.physbaseptr as usize;
        let size = width * height;

        {
            let start_frame = Frame::containing_address(PhysicalAddress::new(start));
            let end_frame = Frame::containing_address(PhysicalAddress::new(start + size * 4 - 1));
            for frame in Frame::range_inclusive(start_frame, end_frame) {
                active_table.identity_map(frame, entry::PRESENT | entry::WRITABLE | entry::NO_EXECUTE);
            }
        }

        memset(start as *mut u8, 0, size * 4);

        *DISPLAY.lock() = Some(Display::new(width, height, slice::from_raw_parts_mut(start as *mut u32, size)));
    }
}

pub unsafe fn init_ap(active_table: &mut ActivePageTable) {
    active_table.identity_map(Frame::containing_address(PhysicalAddress::new(0x5200)), entry::PRESENT | entry::NO_EXECUTE);

    let mode_info = &*(0x5200 as *const VBEModeInfo);
    if mode_info.physbaseptr > 0 {
        let width = mode_info.xresolution as usize;
        let height = mode_info.yresolution as usize;
        let start = mode_info.physbaseptr as usize;
        let size = width * height;

        {
            let start_frame = Frame::containing_address(PhysicalAddress::new(start));
            let end_frame = Frame::containing_address(PhysicalAddress::new(start + size * 4 - 1));
            for frame in Frame::range_inclusive(start_frame, end_frame) {
                active_table.identity_map(frame, entry::PRESENT | entry::WRITABLE | entry::NO_EXECUTE);
            }
        }
    }
}

/// A display
pub struct Display {
    pub width: usize,
    pub height: usize,
    pub data: &'static mut [u32],
    console: Console,
}

impl Display {
    fn new(width: usize, height: usize, data: &'static mut [u32]) -> Self {
        Display {
            width: width,
            height: height,
            data: data,
            console: Console::new(width/8, height/16)
        }
    }

    /// Draw a rectangle
    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let start_y = cmp::min(self.height - 1, y);
        let end_y = cmp::min(self.height, y + h);

        let start_x = cmp::min(self.width - 1, x);
        let len = cmp::min(self.width, x + w) - start_x;

        for y in start_y..end_y {
            let offset = y * self.width + start_x;
            let row = &mut self.data[offset..offset + len];
            for pixel in row.iter_mut() {
                *pixel = color;
            }
        }
    }

    /// Draw a character
    fn char(&mut self, x: usize, y: usize, character: char, color: u32) {
        if x + 8 <= self.width && y + 16 <= self.height {
            let mut offset = y * self.width + x;

            let font_i = 16 * (character as usize);
            if font_i + 16 <= FONT.len() {
                for row in 0..16 {
                    let row_data = FONT[font_i + row];
                    for col in 0..8 {
                        if (row_data >> (7 - col)) & 1 == 1 {
                            self.data[offset + col] = color;
                        }
                    }

                    offset += self.width;
                }
            }
        }
    }

    pub fn write(&mut self, bytes: &[u8]) {
        self.console.write(bytes);
        if self.console.redraw {
            self.console.redraw = false;

            for y in 0..self.console.h {
                if self.console.changed[y] {
                    self.console.changed[y] = false;

                    for x in 0..self.console.w {
                        let block = self.console.display[y * self.console.w + x];

                        let (bg, fg) = if self.console.cursor && self.console.y == y && self.console.x == x {
                            (block.fg.data, block.bg.data)
                        }else{
                            (block.bg.data, block.fg.data)
                        };

                        self.rect(x * 8, y * 16, 8, 16, bg);

                        if block.c != ' ' {
                            self.char(x * 8, y * 16, block.c, fg);
                        }

                        if block.underlined {
                            self.rect(x * 8, y * 16 + 14, 8, 1, fg);
                        }
                    }
                }
            }
        }
    }
}