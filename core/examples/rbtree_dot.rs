use ced::datastruct::RBTree;

fn main() {
    let mut tree = RBTree::new();
    tree.insert(2);
    tree.insert(11);
    tree.insert(6);
    tree.insert(10);
    tree.insert(26);
    tree.insert(7);
    tree.insert(18);
    tree.insert(8);
    tree.insert(13);
    tree.insert(22);
    tree.insert(12);
    tree.insert(15);
    tree.insert(17);
    print!("{}", tree.dump_as_dot());
}
