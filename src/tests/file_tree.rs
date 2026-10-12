use super::*;

#[test]
fn canceled_tree_does_not_deliver_entries_to_the_processor() {
    struct Collector(Vec<std::path::PathBuf>);
    impl FileTreeProcessor for Collector {
        fn process_file(&mut self, path: &Path) {
            self.0.push(path.into());
        }
    }
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("file"), "data").unwrap();
    let mut collector = Collector(Vec::new());
    let token = CancellationToken::new();
    token.cancel();
    FileTree::new(&mut collector)
        .traverse(root.path(), &token)
        .unwrap();
    assert!(collector.0.is_empty());
}
