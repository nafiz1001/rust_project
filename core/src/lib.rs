pub trait ProcessInterface {
    fn new(pid: i64) -> Self;

    fn pid(&self) -> i64;
    fn name(&self) -> String;
    
    fn attach(&self) -> Result<(), String>;
    fn detach(&self) -> Result<(), String>;
    
    fn read_memory<T>(&self, start: usize, buffer: &mut [T]) -> Result<(), String>;
    fn write_memory<T>(&self, start: usize, buffer: &[T]) -> Result<(), String>;
}