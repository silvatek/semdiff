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
    import sys

    count = 10
    if len(sys.argv) > 1:
        try:
            count = int(sys.argv[1])
        except ValueError:
            count = 10

    for n in range(count):
        print(f"{n}: {fibonacci(n)}")


def test_first_values():
    assert fibonacci(0) == 0
    assert fibonacci(1) == 1
    assert fibonacci(2) == 1
    assert fibonacci(10) == 55


# One line below is removed in v2.
# This single line is deleted.

# One line below is changed in v2.
# Count from zero.

# Two lines below are removed in v2.
# This deleted line is first.
# This deleted line is second.

# Two lines are added below in v2.

# Two lines below are changed in v2.
# Old first line.
# Old second line.

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

# A class is added below in v2.
