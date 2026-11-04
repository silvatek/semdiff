import sys


def fibonacci(n):
    if n == 0:
        return 0
    if n == 1:
        return 1
    previous = 0
    current = 1
    for _ in range(2, n + 1):
        next_value = previous + current
        previous = current
        current = next_value
    return current


def main():
    count = 10
    if len(sys.argv) > 1:
        try:
            count = int(sys.argv[1])
        except ValueError:
            pass
    for n in range(count):
        print(f"{n}: {fibonacci(n)}")


def test_first_values():
    assert fibonacci(0) == 0
    assert fibonacci(1) == 1
    assert fibonacci(2) == 1
    assert fibonacci(10) == 55


if __name__ == "__main__":
    main()
