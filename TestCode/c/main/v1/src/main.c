#include <stdio.h>
#include <stdlib.h>

#include "fib.h"

int fibonacci(int n) {
    int previous;
    int current;
    int i;
    int next_value;

    if (n == 0) {
        return 0;
    }
    if (n == 1) {
        return 1;
    }
    previous = 0;
    current = 1;
    for (i = 2; i <= n; i++) {
        next_value = previous + current;
        previous = current;
        current = next_value;
    }
    return current;
}

int main(int argc, char **argv) {
    int parsed;
    int count;
    int n;

    count = 10;
    if (argc > 1) {
        parsed = atoi(argv[1]);
        if (parsed > 0) {
            count = parsed;
        }
    }
    for (n = 0; n < count; n++) {
        printf("%d: %d\n", n, fibonacci(n));
    }
    return 0;
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

// report.c also calls the function above.

// The next line is indented in v2.
void kept(void) {
int value = 1;
}

// The next line gains trailing space in v2.
int trailing = 1;

// The next comment is indented in v2.
// margin

// A blank line is added below in v2.
// A function is added below in v2.

// A structure is added below in v2.

// A function is removed in v2.
int dropped(int n) {
    return n - 1;
}

// A structure is removed in v2.
struct Gone {
    int gone;
};

// An integer changes in v2.
int limit = 10;
