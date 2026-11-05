fn fibonacci(n: u32) -> u64 {
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

fn main() {
    let count = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse::<u32>().ok())
        .unwrap_or(10);

    for n in 0..count {
        println!("{n}: {}", fibonacci(n));
    }
}

#[cfg(test)]
mod tests {
    use super::fibonacci;

    #[test]
    fn first_values() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
        assert_eq!(fibonacci(2), 1);
        assert_eq!(fibonacci(10), 55);
    }
}

// One line below is removed in v2.
// This single line is deleted.

// One line below is changed in v2.
// Count from zero.

// Two lines below are removed in v2.
// This deleted line is first.
// This deleted line is second.

// Two lines are added below in v2.

// Two lines below are changed in v2.
// Old first line.
// Old second line.

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

// A structure is added below in v2.
