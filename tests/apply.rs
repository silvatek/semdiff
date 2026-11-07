//! Tests of replaying a semdif onto one tree.

use crate::filesystem::Tree;

#[test]
fn applies_a_rename_inside_a_directory() {
    let tree = Tree::new();
    let quote = "Brevity is the soul of wit.\n";
    let source = "fn fibonacci() {}\n";
    tree.write("left/quote.txt", quote);
    tree.write("right/notes/quotation.txt", quote);
    tree.write("left/src/main.rs", source);
    tree.write("right/src/main.rs", source);
    tree.write("edited/quote.txt", quote);
    tree.write("edited/src/main.rs", source);

    let document = tree.compare("left", "right");
    tree.apply("edited", &document).unwrap();

    assert!(!tree.exists("edited/quote.txt"));
    assert_eq!(tree.read("edited/notes/quotation.txt"), quote.as_bytes());
    assert_eq!(tree.read("edited/src/main.rs"), source.as_bytes());
}
