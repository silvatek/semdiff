def fib(n):
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


# Print Fibonacci numbers.
def main():
    import sys

    count = 10
    if len(sys.argv) > 1:
        try:
            count = int(sys.argv[1])
        except ValueError:
            count = 10

    for n in range(count):
        print(f"{n}: {fib(n)}")


def test_first_values():
    assert fib(0) == 0
    assert fib(1) == 1
    assert fib(2) == 1
    assert fib(10) == 55


# One line below is removed in v2.

# One line below is changed in v2.
# Count from one.

# Two lines below are removed in v2.

# Two lines are added below in v2.
# This added line is first.
# This added line is second.

# Two lines below are changed in v2.
# New first line.
# New second line.

# report.py also calls the function above.
import report


if __name__ == "__main__":
    main()

# The next line is indented in v2.
def kept():
        x = 1

# The next line gains trailing space in v2.
trailing = 1 

# The next comment is indented in v2.
    # margin

# A blank line is added below in v2.

# A function is added below in v2.
def shifted(n):
    return n + 1

# A class is added below in v2.
class Step:
    by = 1

# A method is added below in v2.
class Count:
    by = 1
    def step(self):
        return self.by
