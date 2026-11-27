
All Rust code should be formatted with `cargo fmt` before committing.

All public values and functions should have doc comments. Comments should say what something does, and why. They should generally not cover details of how things are done.

Code should be designed for extensibility, preferably using traits to extract commonality from related code.

Functions should be short with minimal nesting; a useful pattern for loops is to have a function to act on individual structures, and then have another function with a pluralised name to call the inner function for all items in the list. For example `process_thing(item: Item)` called by `process_things(items: &[Item])`. Extracting small sections of code into functions allows us to name that code, preferably with a name that explains what the function does (not how).