# semdiff

## Dialects

A dialect describes one language: which files belong to it, how comments are written, and how a block of code starts and ends. Rust and Python have dialects. Classification and merging will use that description.

- A change whose only difference is insignificant whitespace is flagged as whitespace. Rust marks blocks with `{` and `}`, so an indent-only edit is whitespace. Python marks blocks by indentation, so an indent change is a block-boundary edit.
- A change that touches only comments is flagged as a comment. Rust comments start with `//`, or run from `/*` to `*/`. Python comments start with `#` and end at the end of the line. A triple-quoted string is a string, so editing one is a string edit.
- Adjacent edits are merged while they stay inside one block. A block edge stops the merge: `{` and `}` in Rust, and an indent change in Python. An edit that spans two functions stays two changes.
- An added run of lines whose first line opens a named block is `named-block-added`. Rust treats `fn`, `struct`, `enum`, `union`, `trait`, and `impl` as that opening, with `pub`, `async`, `unsafe`, or `const` in front. The name is the next identifier. Python does not record named blocks yet.

## License

Licensed under the [Apache License 2.0](LICENSE).
