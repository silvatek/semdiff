#include <stdio.h>
#include <stdlib.h>

#include "fib.h"

int fib(int n) {
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

// Print Fibonacci numbers.
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
        printf("%d: %d\n", n, fib(n));
    }
    return 0;
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
int shifted(int n) {
    return n + 1;
}

// A structure is added below in v2.
struct Step {
    int by;
};

// A function is removed in v2.

// A structure is removed in v2.

// An integer changes in v2.
int limit = 20;
