fn fib(n: u32) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => {
            let mut previous = 0u64;
            let mut current = 1u64;
            for _ in 2..=n {
                let next = previous + current;
                previous = current;
                current = next;
            }
            current
        }
    }
}

// Print Fibonacci numbers.
fn main() {
    let count = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse::<u32>().ok())
        .unwrap_or(10);

    for n in 0..count {
        println!("{n}: {}", fib(n));
    }
}

#[cfg(test)]
mod tests {
    use super::fib;

    #[test]
    fn first_values() {
        assert_eq!(fib(0), 0);
        assert_eq!(fib(1), 1);
        assert_eq!(fib(2), 1);
        assert_eq!(fib(10), 55);
    }
}

// One line below is removed in v2.

// One line below is changed in v2.
// Count from one.

// Two lines below are removed in v2.

// Two lines are added below in v2.
// This added line is first.
// This added line is second.

// Two lines below are changed in v2.
// New first line.
// New second line.

// report.rs also calls the function above.
mod report;

// The next line is indented in v2.
fn kept() {
    let value = 1;
}

// The next line gains trailing space in v2.
fn marked() {
    let trailing = 1; 
}

// A blank line is added below in v2.

// A function is added below in v2.
fn shifted(n: u32) -> u64 {
    n + 1
}

// A structure is added below in v2.
struct Step {
    by: u32,
}

// A method is added below in v2.
struct Count {
    by: u32,
}

impl Count {
    fn step(&self) -> u32 {
        self.by
    }
}

// A function is removed in v2.

// A structure is removed in v2.

// A method is removed in v2.
struct Total {
    left: u32,
}

impl Total {
}

// An integer changes in v2.
const LIMIT: usize = 20;
