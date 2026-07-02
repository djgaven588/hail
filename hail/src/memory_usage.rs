use crate::{FunctionEntry, VecIterator};

/// Annoyingly a required implementation in order to enforce memory limits
/// This should be a "scripter friendly" value that behaves the same regardless of compilation target (don't use size_of::<usize>, use size_of::<u64>, lock it down)
/// The default implementation is fine for anything where the size is known exactly upfront, but anything with dynamic allocation should implement it's own usage.
/// The value here isn't meant to be *perfect*, just close enough to prevent scripts from using orders of magnitude more memory than the memory limits.
pub trait MemoryUsage {
    fn memory_usage(&self) -> usize {
        std::mem::size_of_val(self)
    }
}

impl MemoryUsage for () {}
impl MemoryUsage for bool {}
impl MemoryUsage for i8 {}
impl MemoryUsage for u8 {}
impl MemoryUsage for i16 {}
impl MemoryUsage for u16 {}
impl MemoryUsage for f32 {}
impl MemoryUsage for i32 {}
impl MemoryUsage for u32 {}
impl MemoryUsage for f64 {}
impl MemoryUsage for i64 {}
impl MemoryUsage for u64 {}

impl MemoryUsage for String {
    fn memory_usage(&self) -> usize {
        // Len here is bytes as String's internals are bytes, not chars
        // NOTE: Fixed count to u64 instead of usize for cross platform consistency
        std::mem::size_of::<u64>() + self.len()
    }
}

impl MemoryUsage for FunctionEntry {
    fn memory_usage(&self) -> usize {
        // NOTE: Should this be higher? Yes. I've done it like this to be more like a "pointer cost" to be fair to the script writer
        // NOTE: Fixed "pointer" to u64 instead of usize for cross platform consistency
        self.name.memory_usage() + std::mem::size_of::<u64>()
    }
}

impl<T: MemoryUsage> MemoryUsage for Option<T> {
    fn memory_usage(&self) -> usize {
        // NOTE: Should this be higher? Yes. I've done it like this to a "is it there" + value usage to be fair to the script writer
        std::mem::size_of::<bool>() + self.as_ref().map_or(0, |v| v.memory_usage())
    }
}

impl<T: MemoryUsage> MemoryUsage for Vec<T> {
    fn memory_usage(&self) -> usize {
        // NOTE: Fixed counter to u64 instead of usize for cross platform consistency
        std::mem::size_of::<u64>()
            + self.iter().map(|v| v.memory_usage()).sum::<usize>()
            + (self.capacity() - self.len()) * std::mem::size_of::<T>()
    }
}

impl<T: MemoryUsage> MemoryUsage for VecIterator<T> {
    fn memory_usage(&self) -> usize {
        // NOTE: Should this be higher? Yes. I've done it like this to a index + inner vec to be fair to the script writer
        // NOTE: Fixed index to u64 instead of usize for cross platform consistency
        std::mem::size_of::<u64>() + self.values.memory_usage()
    }
}

impl<T: MemoryUsage> MemoryUsage for std::ops::Range<T> {
    fn memory_usage(&self) -> usize {
        self.start.memory_usage() + self.end.memory_usage()
    }
}

impl<T: MemoryUsage> MemoryUsage for crate::module::RangeIterator<T> {
    fn memory_usage(&self) -> usize {
        self.current.memory_usage() + self.end.memory_usage()
    }
}
