//! The process entry. Behavior of the command belongs in the library.

fn main() -> std::process::ExitCode {
    semdiff::run()
}
