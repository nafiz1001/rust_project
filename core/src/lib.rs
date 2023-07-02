use core::ops::Range;

pub type PID = i64;

pub enum MemoryPermission {
    READONLY,
    READWRITE,
    NONE,
}

pub enum MemoryKind {
    STATIC,
    STACK,
    HEAP,
    UNKNOWN,
}

pub struct MemoryRegion {
    pub range: Range<usize>,
    pub permission: MemoryPermission,
    pub kind: MemoryKind,
}

pub trait Process {
    fn new(pid: PID) -> Self;
    fn pid(&self) -> PID;
    fn name(&self) -> String;
    fn attach(&self) -> Result<(), String>;
    fn detach(&self) -> Result<(), String>;
    fn read_memory<T>(&self, offset: usize,  buffer: *mut T) -> Result<(), String>;
    fn read_memory_slice<T>(&self, offset: usize,  buffer: &mut [T]) -> Result<(), String>;
    fn write_memory<T>(&self, offset: usize, buffer: *const T) -> Result<(), String>;
    fn write_memory_slice<T>(&self, offset: usize, buffer: &[T]) -> Result<(), String>;
}

pub trait MemoryRegionIterator<'a, P: Process>: Iterator<Item = MemoryRegion> {
    fn new(process: &'a P, offset: usize, limit: usize) -> Self;
}

pub trait ProcessIterator<P: Process>: Iterator<Item = P> {
}

