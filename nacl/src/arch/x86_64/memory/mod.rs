pub mod allocator;
pub mod mapper;

use alloc::slice;
use core::iter::{Filter, FlatMap, Map, StepBy};
use core::ops::Range;

use limine::memory_map::{Entry, EntryType};

use x86_64::structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}

/// Initialize the heap.
///
/// # SAFETY
///
/// the physical memory offset must be valid.
pub unsafe fn init(physical_memory_offset: VirtAddr, memory_regions: &'static [&'static Entry]) {
    let level_4_table = active_level_4_table(physical_memory_offset);
    let mut page_table = OffsetPageTable::new(level_4_table, physical_memory_offset);
    let mut frame_allocator = BootInfoFrameAllocator::init(memory_regions);
    allocator::init_heap(&mut page_table, &mut frame_allocator)
        .expect("heap initialization failed");
}

type FilterFn = fn(&&&Entry) -> bool;
type FlatMapFn = fn(&&Entry) -> StepBy<Range<u64>>;
type MapFn = fn(u64) -> PhysFrame;
type UsableFrames = Map<
    FlatMap<Filter<slice::Iter<'static, &'static Entry>, FilterFn>, StepBy<Range<u64>>, FlatMapFn>,
    MapFn,
>;

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    frames: UsableFrames,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(regions: &'static [&'static Entry]) -> Self {
        let f1: FilterFn = |x| x.entry_type == EntryType::USABLE;
        let f2: FlatMapFn = |x| (x.base..x.base + x.length).step_by(4096);
        BootInfoFrameAllocator {
            frames: regions
                .iter()
                .filter(f1)
                .flat_map(f2)
                .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr))),
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.frames.next()
    }
}
