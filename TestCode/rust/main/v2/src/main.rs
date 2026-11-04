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
