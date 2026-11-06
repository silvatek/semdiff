function fib(n) {
    if (n === 0) {
        return 0;
    }
    if (n === 1) {
        return 1;
    }
    let previous = 0;
    let current = 1;
    for (let i = 2; i <= n; i++) {
        const nextValue = previous + current;
        previous = current;
        current = nextValue;
    }
    return current;
}

// Print Fibonacci numbers.
function main() {
    const parsed = Number.parseInt(process.argv[2], 10);
    const count = Number.isNaN(parsed) ? 10 : parsed;

    for (let n = 0; n < count; n++) {
        console.log(`${n}: ${fib(n)}`);
    }
}

function testFirstValues() {
    console.assert(fib(0) === 0);
    console.assert(fib(1) === 1);
    console.assert(fib(2) === 1);
    console.assert(fib(10) === 55);
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

// report.js also calls the function above.
import * as report from "./report.js";

main();

// The next line is indented in v2.
function kept() {
    let value = 1;
}

// The next line gains trailing space in v2.
const trailing = 1; 

// The next comment is indented in v2.
    // margin

// A blank line is added below in v2.

// A function is added below in v2.
function shifted(n) {
    return n + 1;
}

// A class is added below in v2.
class Step {
    by = 1;
}

// A method is added below in v2.
class Count {
    by = 1;
    step() {
        return this.by;
    }
}

// A function is removed in v2.

// A class is removed in v2.

// A method is removed in v2.
class Total {
    left = 0;
}

// An integer changes in v2.
const LIMIT = 20;
