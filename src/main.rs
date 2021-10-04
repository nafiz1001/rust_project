use linux_bindings::ProcessIterator;

fn main() {
    for proc in ProcessIterator::new() {
        println!(
            "{}\t{}",
            proc.pid(),
            proc.name()
        );
    }
}
