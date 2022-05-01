use std::ops::Range;

pub trait ProcessTrait {
    fn new(pid: i64) -> Self;

    fn pid(&self) -> i64;
    fn name(&self) -> String;

    fn attach(&self) -> Result<(), String>;
    fn detach(&self) -> Result<(), String>;

    fn read_memory<T>(&self, start: usize, buffer: &mut [T]) -> Result<(), String>;
    fn write_memory<T>(&self, start: usize, buffer: &[T]) -> Result<(), String>;
}

pub enum MemoryPermission {
    READONLY,
    READWRITE,
    NONE,
}

pub enum MemoryKind {
    STACK,
    HEAP,
    UNKNOWN,
}

pub struct MemoryRegion {
    pub range: Range<usize>,
    pub permission: MemoryPermission,
    pub kind: MemoryKind,
}
