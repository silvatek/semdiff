function fibonacci(n) {
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

function main() {
    const parsed = Number.parseInt(process.argv[2], 10);
    const count = Number.isNaN(parsed) ? 10 : parsed;

    for (let n = 0; n < count; n++) {
        console.log(`${n}: ${fibonacci(n)}`);
    }
}

function testFirstValues() {
    console.assert(fibonacci(0) === 0);
    console.assert(fibonacci(1) === 1);
    console.assert(fibonacci(2) === 1);
    console.assert(fibonacci(10) === 55);
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

// A class is added below in v2.

// A method is added below in v2.
class Count {
    by = 1;
}

// A function is removed in v2.
function dropped(n) {
    return n - 1;
}

// A class is removed in v2.
class Gone {
    gone = 2;
}

// A method is removed in v2.
class Total {
    left = 0;
    undo() {
        return this.left;
    }
}

// An integer changes in v2.
const LIMIT = 10;
