use std::cell::RefCell;
use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Colour {
    Black,
    Red,
}

impl fmt::Display for Colour {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Colour::*;
        match self {
            Black => write!(f, "black"),
            Red => write!(f, "red"),
        }
    }
}

pub struct NodeData<T: fmt::Debug + Ord> {
    colour: Colour,
    parent: Option<Node<T>>,
    left: Option<Node<T>>,
    right: Option<Node<T>>,
    data: T,
}

impl<T> NodeData<T>
where
    T: fmt::Debug + Ord,
{
    fn new(data: T) -> NodeData<T> {
        NodeData {
            colour: Colour::Red,
            parent: None,
            left: None,
            right: None,
            data,
        }
    }
}

pub struct Node<T: fmt::Debug + Ord>(Rc<RefCell<NodeData<T>>>);

impl<T> Node<T>
where
    T: fmt::Debug + Ord,
{
    fn id(&self) -> String {
        let address = format!("{:?}", self.0.as_ptr());
        address[2..].to_owned()
    }

    fn duplicate(&self) -> Node<T> {
        Node(Rc::clone(&self.0))
    }

    pub fn data(&self) -> T
    where
        T: Clone,
    {
        self.borrow().data.clone()
    }

    fn swap_data(&mut self, other: &mut Node<T>) {
        std::mem::swap(&mut self.borrow_mut().data, &mut other.borrow_mut().data)
    }

    fn parent(&self) -> Option<Node<T>> {
        self.borrow().parent.as_ref().map(Node::duplicate)
    }

    fn set_parent<I>(&mut self, node: I)
    where
        I: Into<Option<Node<T>>>,
    {
        self.borrow_mut().parent = node.into()
    }

    fn left(&self) -> Option<Node<T>> {
        self.borrow().left.as_ref().map(Node::duplicate)
    }

    fn set_left<I>(&mut self, node: I)
    where
        I: Into<Option<Node<T>>>,
    {
        self.borrow_mut().left = node.into()
    }

    fn right(&self) -> Option<Node<T>> {
        self.borrow().right.as_ref().map(Node::duplicate)
    }

    fn set_right<I>(&mut self, node: I)
    where
        I: Into<Option<Node<T>>>,
    {
        self.borrow_mut().right = node.into()
    }

    fn is_left_child(&self) -> bool {
        self.parent()
            .as_ref()
            .and_then(Node::left)
            .as_ref()
            .map(|n| n == self)
            .unwrap_or(false)
    }

    fn sibling(&self) -> Option<Node<T>> {
        if self.is_left_child() {
            self.parent()?.right()
        } else {
            self.parent()?.left()
        }
    }

    fn uncle(&self) -> Option<Node<T>> {
        self.parent()?.sibling()
    }

    fn colour(&self) -> Colour {
        self.borrow().colour
    }

    fn set_colour(&mut self, colour: Colour) {
        self.borrow_mut().colour = colour;
    }

    pub fn iter(&self) -> Iter<T> {
        Iter {
            cursor: Some(self.duplicate()),
        }
    }
}

impl<T> From<T> for Node<T>
where
    T: fmt::Debug + Ord,
{
    fn from(data: T) -> Node<T> {
        Node(Rc::new(RefCell::new(NodeData::new(data))))
    }
}

impl<T> Deref for Node<T>
where
    T: fmt::Debug + Ord,
{
    type Target = Rc<RefCell<NodeData<T>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> fmt::Debug for Node<T>
where
    T: fmt::Debug + Ord,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Node {{id: {}, p: {:?}, l: {:?}, r: {:?}, data: \"{:?}\"}}",
            self.id(),
            self.parent().as_ref().map(Node::id),
            self.left().as_ref().map(Node::id),
            self.right().as_ref().map(Node::id),
            self.borrow().data,
        )
    }
}

impl<T> PartialEq for Node<T>
where
    T: fmt::Debug + Ord,
{
    fn eq(&self, other: &Node<T>) -> bool {
        Rc::ptr_eq(self, other)
    }
}

#[derive(Default)]
pub struct RBTree<T: fmt::Debug + Ord> {
    root: Option<Node<T>>,
}

impl<T> RBTree<T>
where
    T: fmt::Debug + Ord,
{
    pub fn new() -> RBTree<T> {
        RBTree { root: None }
    }

    fn insert_from(&mut self, mut root: Node<T>, data: T) -> Option<Node<T>> {
        if data == root.borrow().data {
            None
        } else if data <= root.borrow().data {
            if root.left().is_none() {
                let mut node = Node::from(data);
                node.set_parent(root.duplicate());
                root.set_left(node.duplicate());
                Some(node)
            } else {
                self.insert_from(root.left().as_ref().unwrap().duplicate(), data)
            }
        } else if root.right().is_none() {
            let mut node = Node::from(data);
            node.set_parent(root.duplicate());
            root.set_right(node.duplicate());
            Some(node)
        } else {
            self.insert_from(root.right().as_ref().unwrap().duplicate(), data)
        }
    }

    fn rotate_right(&mut self, mut node: Node<T>) {
        trace!("rotate right: {:?}", node);
        let mut parent = node.left().expect("get parent node");
        node.set_left(parent.right());
        if let Some(ref mut right) = parent.right() {
            right.set_parent(node.duplicate());
        }
        parent.set_right(node.duplicate());
        parent.set_parent(node.parent());
        if let Some(ref mut gparent) = parent.parent() {
            if node.is_left_child() {
                gparent.set_left(parent.duplicate());
            } else {
                gparent.set_right(parent.duplicate());
            }
        } else {
            self.root = Some(parent.duplicate());
        }
        node.set_parent(parent);
    }

    fn rotate_left(&mut self, mut node: Node<T>) {
        trace!("rotate left: {:?}", node);
        let mut parent = node.right().expect("get parent node");
        node.set_right(parent.left());
        if let Some(ref mut left) = parent.left() {
            left.set_parent(node.duplicate());
        }
        parent.set_left(node.duplicate());
        parent.set_parent(node.parent());
        if let Some(ref mut gparent) = parent.parent() {
            if node.is_left_child() {
                gparent.set_left(parent.duplicate());
            } else {
                gparent.set_right(parent.duplicate());
            }
        } else {
            self.root = Some(parent.duplicate());
        }
        node.set_parent(parent);
    }

    fn balance(&mut self, mut node: Node<T>) {
        if node.parent().is_none() {
            trace!("balance root: {:?}", node);
            node.set_colour(Colour::Black);
        } else if node.parent().as_ref().map(Node::colour) == Some(Colour::Black) {
            trace!("balance black parent: {:?}", node);
        // we're good here
        } else if node.uncle().as_ref().map(Node::colour) == Some(Colour::Red) {
            trace!("balance red uncle: {:?}", node);
            // parent colour <- black
            node.parent().as_mut().unwrap().set_colour(Colour::Black);
            // uncle colour <- black
            node.uncle().as_mut().unwrap().set_colour(Colour::Black);
            // grand parent colour <- red
            let mut grand_parent = node.parent().as_ref().and_then(Node::parent).unwrap();
            grand_parent.set_colour(Colour::Red);
            // balance from grand parent
            self.balance(grand_parent.duplicate());
        } else {
            trace!("balance black uncle: {:?}", node);
            let parent = node.parent().as_ref().map(Node::duplicate).unwrap();
            let mut new_node = node.duplicate();

            // rotate as needed
            let parent_is_left = parent.is_left_child();
            let node_is_left = node.is_left_child();
            if parent_is_left && !node_is_left {
                self.rotate_left(node.parent().as_ref().unwrap().duplicate());
                new_node = node.left().as_ref().unwrap().duplicate();
            } else if !parent_is_left && node_is_left {
                self.rotate_right(node.parent().as_ref().unwrap().duplicate());
                new_node = node.right().as_ref().unwrap().duplicate();
            }

            let mut new_gparent = parent.parent().as_ref().map(Node::duplicate).unwrap();

            // swap parent and grand parent colours
            new_node
                .parent()
                .as_ref()
                .map(Node::duplicate)
                .unwrap()
                .set_colour(Colour::Black);
            new_gparent.set_colour(Colour::Red);

            if new_node.is_left_child() {
                self.rotate_right(new_gparent.duplicate());
            } else {
                self.rotate_left(new_gparent.duplicate());
            }
        }
    }

    pub fn insert(&mut self, data: T) -> Option<Node<T>> {
        trace!("insert {:?}", data);
        let node = if let Some(ref root) = self.root {
            self.insert_from(root.duplicate(), data)
        } else {
            self.root = Some(Node::from(data));
            Some(self.root.as_ref().unwrap().duplicate())
        };
        if let Some(ref n) = node {
            self.balance(n.duplicate());
        }
        node
    }

    pub fn first(&self) -> Option<Node<T>> {
        let mut n = self.root.as_ref().map(Node::duplicate)?;
        while let Some(left) = n.left() {
            n = left;
        }
        Some(n)
    }

    pub fn last(&self) -> Option<Node<T>> {
        let mut n = self.root.as_ref().map(Node::duplicate)?;
        while let Some(right) = n.right() {
            n = right;
        }
        Some(n)
    }

    fn successor(node: Node<T>) -> Option<Node<T>> {
        if let Some(right) = node.right() {
            let mut tmp = right;
            while let Some(n) = tmp.left() {
                tmp = n;
            }
            Some(tmp)
        } else if node.is_left_child() {
            node.parent()
        } else {
            let mut tmp = node.duplicate();
            while tmp.parent().as_ref().and_then(Node::right).as_ref() == Some(&tmp) {
                tmp = tmp.parent().as_ref().unwrap().duplicate();
            }
            tmp.parent()
        }
    }

    pub fn get(&self, data: &T) -> Option<Node<T>> {
        trace!("get {:?}", data);
        let mut tmp = self.root.as_ref().map(Node::duplicate);
        while let Some(ref n) = tmp {
            if *data == n.borrow().data {
                return Some(n.duplicate());
            } else if *data < n.borrow().data {
                tmp = n.left();
            } else {
                tmp = n.right();
            }
        }
        None
    }

    fn delete_fixup(&mut self, node: Node<T>) {
        let mut n = Some(node.duplicate());
        while n != self.root && n.as_ref().map(Node::colour) == Some(Colour::Black) {
            if n.as_ref().unwrap().is_left_child() {
                let mut sibling = n.as_ref().unwrap().sibling().unwrap();
                if sibling.colour() == Colour::Red {
                    let mut parent = n.as_ref().unwrap().parent().unwrap().duplicate();
                    sibling.set_colour(Colour::Black);
                    parent.set_colour(Colour::Red);
                    self.rotate_left(parent.duplicate());
                    sibling = parent.right().unwrap().duplicate();
                }
                if sibling.left().as_ref().map(Node::colour) == Some(Colour::Black)
                    && sibling.right().as_ref().map(Node::colour) == Some(Colour::Black)
                {
                    sibling.set_colour(Colour::Red);
                    n = n.as_ref().unwrap().parent().as_ref().map(Node::duplicate);
                } else if sibling.right().as_ref().map(Node::colour) == Some(Colour::Black) {
                    sibling.left().as_mut().unwrap().set_colour(Colour::Black);
                    sibling.set_colour(Colour::Red);
                    self.rotate_right(sibling.duplicate());
                    sibling = n
                        .as_ref()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .right()
                        .unwrap()
                        .duplicate();
                }
                sibling.set_colour(n.as_ref().unwrap().parent().as_ref().unwrap().colour());
                n.as_ref()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .set_colour(Colour::Black);
                sibling.right().as_mut().unwrap().set_colour(Colour::Black);
                self.rotate_left(n.as_ref().unwrap().parent().as_ref().unwrap().duplicate());
                n = self.root.as_ref().map(Node::duplicate);
            } else {
                let mut sibling = n.as_ref().unwrap().sibling().unwrap();
                if sibling.colour() == Colour::Red {
                    let parent = n.as_ref().unwrap().parent().unwrap().duplicate();
                    sibling.set_colour(Colour::Black);
                    n.as_ref()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .set_colour(Colour::Red);
                    self.rotate_right(parent.duplicate());
                    sibling = parent.left().unwrap().duplicate();
                }
                if sibling.right().as_ref().map(Node::colour) == Some(Colour::Black)
                    && sibling.left().as_ref().map(Node::colour) == Some(Colour::Black)
                {
                    sibling.set_colour(Colour::Red);
                    n = n.as_ref().unwrap().parent().as_ref().map(Node::duplicate);
                } else if sibling.left().unwrap().colour() == Colour::Black {
                    sibling.right().unwrap().set_colour(Colour::Black);
                    sibling.set_colour(Colour::Red);
                    self.rotate_left(sibling.duplicate());
                    sibling = n
                        .as_ref()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .left()
                        .unwrap()
                        .duplicate();
                }
                sibling.set_colour(n.as_ref().unwrap().parent().unwrap().colour());
                n.as_ref()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .set_colour(Colour::Black);
                sibling.left().unwrap().set_colour(Colour::Black);
                self.rotate_right(n.unwrap().parent().unwrap().duplicate());
                n = self.root.as_ref().map(Node::duplicate);
            }
        }
        if let Some(ref mut node) = n {
            node.set_colour(Colour::Black);
        }
    }

    fn delete(&mut self, node: Node<T>) {
        trace!("delete {:?}", node);
        let mut n = if node.left().is_none() || node.right().is_none() {
            node.duplicate()
        } else {
            Self::successor(node.duplicate()).unwrap()
        };
        let mut replacer = if n.left().is_some() {
            n.left()
        } else {
            n.right()
        };
        if let Some(ref mut r) = replacer {
            r.set_parent(n.parent());
        }
        if let Some(ref mut parent) = n.parent() {
            if n.is_left_child() {
                parent.set_left(replacer.as_ref().map(Node::duplicate));
            } else {
                parent.set_right(replacer.as_ref().map(Node::duplicate));
            }
        } else {
            self.root = replacer.as_ref().map(Node::duplicate);
        }
        if n != node {
            n.swap_data(&mut node.duplicate())
        }
        if n.colour() == Colour::Black && replacer.is_some() {
            self.delete_fixup(replacer.as_ref().map(Node::duplicate).unwrap());
        }
    }

    pub fn remove(&mut self, data: &T) -> bool {
        match self.get(data) {
            Some(node) => {
                self.delete(node);
                true
            }
            None => false,
        }
    }

    pub fn iter(&self) -> Iter<T> {
        Iter {
            cursor: self.first(),
        }
    }

    pub fn values(&self) -> Values<T>
    where
        T: Clone,
    {
        Values { inner: self.iter() }
    }

    pub fn dump_as_dot(&self) -> String {
        let mut lines = Vec::new();
        lines.push(String::from("graph Tree {"));

        let mut definitions = Vec::new();
        let mut links = Vec::new();
        let mut tmp = self.first();
        while let Some(ref node) = tmp {
            definitions.push(format!(
                "    Node{} [label=\"{:?}\", color={}]",
                node.id(),
                node.borrow().data,
                node.colour()
            ));
            if node.left().is_some() {
                links.push(format!(
                    "    Node{} -- Node{}",
                    node.id(),
                    node.left().as_ref().unwrap().id()
                ));
            } else {
                definitions.push(format!("    NullL{} [shape=point]", node.id()));
                links.push(format!("    Node{0} -- NullL{0}", node.id()));
            }
            if node.right().is_some() {
                links.push(format!(
                    "    Node{} -- Node{}",
                    node.id(),
                    node.right().as_ref().unwrap().id()
                ));
            } else {
                definitions.push(format!("    NullR{} [shape=point]", node.id()));
                links.push(format!("    Node{0} -- NullR{0}", node.id()));
            }
            tmp = Self::successor(node.duplicate());
        }

        lines.append(&mut definitions);
        lines.push(String::new());
        lines.append(&mut links);

        lines.push(String::from("}"));
        lines.push(String::new());
        lines.join("\n")
    }

    fn clone_subtree(node: Option<Node<T>>) -> Option<Node<T>>
    where
        T: Clone,
    {
        let sub = node?;

        let mut cloned = Node::from(sub.data());
        cloned.set_colour(sub.colour());
        cloned.set_left(Self::clone_subtree(sub.left()));
        cloned.set_right(Self::clone_subtree(sub.right()));
        if let Some(ref mut left) = cloned.left() {
            left.set_parent(cloned.duplicate());
        }
        if let Some(ref mut right) = cloned.right() {
            right.set_parent(cloned.duplicate());
        }
        Some(cloned)
    }
}

impl<T> Clone for RBTree<T>
where
    T: Clone + fmt::Debug + Ord,
{
    fn clone(&self) -> Self {
        RBTree {
            root: Self::clone_subtree(self.root.as_ref().map(Node::duplicate)),
        }
    }
}

pub struct Iter<T>
where
    T: fmt::Debug + Ord,
{
    cursor: Option<Node<T>>,
}

impl<T> Iterator for Iter<T>
where
    T: fmt::Debug + Ord,
{
    type Item = Node<T>;

    fn next(&mut self) -> Option<Node<T>> {
        let node = self.cursor.as_ref().map(Node::duplicate);
        if let Some(ref n) = self.cursor {
            self.cursor = RBTree::successor(n.duplicate());
        }
        node
    }
}

pub struct Values<T>
where
    T: Clone + fmt::Debug + Ord,
{
    inner: Iter<T>,
}

impl<T> Iterator for Values<T>
where
    T: Clone + fmt::Debug + Ord,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.inner.next().as_ref().map(Node::data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_node {
        ($node:expr, NULL) => {
            assert!(node.is_none())
        };
        ($node:expr, $data:expr) => {
            assert_eq!($node.as_ref().unwrap().borrow().data, $data);
        };
        ($node:expr, $data:expr, $colour:expr) => {
            assert_eq!($node.as_ref().unwrap().borrow().data, $data);
            assert_eq!($node.as_ref().unwrap().colour(), $colour);
        };
    }

    #[test]
    fn rotate_left_root() {
        let mut tree = RBTree::new();
        tree.insert(2);
        tree.insert(11);
        tree.insert(15);

        print!("{}", tree.dump_as_dot());
        assert_node!(tree.root, 11, Colour::Black);
        assert_node!(tree.root.as_ref().unwrap().left(), 2, Colour::Red);
        assert_node!(tree.root.as_ref().unwrap().right(), 15, Colour::Red);
    }

    #[test]
    fn rotate_left_parent() {
        let mut tree = RBTree::new();
        tree.insert(3);
        tree.insert(6);
        tree.insert(2);
        tree.insert(11);
        tree.insert(15);

        print!("{}", tree.dump_as_dot());
        assert_node!(tree.root, 3, Colour::Black);
        assert_node!(tree.root.as_ref().unwrap().left(), 2, Colour::Black);
        assert_node!(tree.root.as_ref().unwrap().right(), 11, Colour::Black);
    }

    #[test]
    fn rotate_right_root() {
        let mut tree = RBTree::new();
        tree.insert(11);
        tree.insert(6);
        tree.insert(2);

        print!("{}", tree.dump_as_dot());
        assert_node!(tree.root, 6, Colour::Black);
        assert_node!(tree.root.as_ref().unwrap().left(), 2, Colour::Red);
        assert_node!(tree.root.as_ref().unwrap().right(), 11, Colour::Red);
    }

    #[test]
    fn rotate_right_parent() {
        let mut tree = RBTree::new();
        tree.insert(11);
        tree.insert(6);
        tree.insert(15);
        tree.insert(3);
        tree.insert(2);

        print!("{}", tree.dump_as_dot());
        assert_node!(tree.root, 11, Colour::Black);
        assert_node!(tree.root.as_ref().unwrap().left(), 3, Colour::Black);
        assert_node!(tree.root.as_ref().unwrap().right(), 15, Colour::Black);
    }

    #[test]
    fn insert() {
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

        print!("{}", tree.dump_as_dot());
        assert_eq!(
            tree.values().collect::<Vec<i32>>(),
            vec![2, 6, 7, 8, 10, 11, 13, 18, 22, 26]
        );
    }

    #[test]
    fn delete_pseudoleaves() {
        let mut tree = RBTree::new();
        tree.insert(50);
        tree.insert(20);
        tree.insert(60);
        tree.insert(30);
        tree.insert(40);
        tree.insert(70);
        tree.insert(80);

        tree.remove(&20);
        assert_eq!(
            tree.values().collect::<Vec<i32>>(),
            vec![30, 40, 50, 60, 70, 80]
        );

        tree.remove(&30);
        assert_eq!(
            tree.values().collect::<Vec<i32>>(),
            vec![40, 50, 60, 70, 80]
        );

        tree.remove(&80);
        assert_eq!(tree.values().collect::<Vec<i32>>(), vec![40, 50, 60, 70]);

        tree.remove(&70);
        assert_eq!(tree.values().collect::<Vec<i32>>(), vec![40, 50, 60]);
    }

    #[test]
    fn delete() {
        let mut keep = Vec::new();
        let mut remove = Vec::new();
        for i in (1..30).step_by(3) {
            keep.push(i);
            remove.push(i + 2);
        }

        let mut tree = RBTree::new();
        for i in remove.iter().rev() {
            tree.insert(*i);
        }
        for i in &keep {
            tree.insert(*i);
        }
        for i in remove {
            tree.remove(&i);
        }

        print!("{}", tree.dump_as_dot());
        assert_eq!(tree.values().collect::<Vec<i32>>(), keep);
    }

    #[test]
    fn first_and_last() {
        let mut tree = RBTree::new();
        assert!(tree.first().is_none());
        assert!(tree.last().is_none());

        tree.insert(50);
        let mut tree = RBTree::new();
        assert_eq!(tree.first(), tree.last());

        tree.insert(20);
        tree.insert(60);
        tree.insert(30);
        tree.insert(40);
        tree.insert(70);
        tree.insert(80);

        print!("{}", tree.dump_as_dot());
        assert_eq!(tree.first().unwrap().data(), 20);
        assert_eq!(tree.last().unwrap().data(), 80);
    }

    #[test]
    fn find() {
        let mut tree = RBTree::new();
        assert!(tree.insert(2).is_some());
        tree.insert(13);
        assert!(tree.insert(2).is_none());
        tree.insert(22);
        assert!(tree.insert(2).is_none());

        print!("{}", tree.dump_as_dot());
        assert_eq!(tree.get(&2).unwrap().data(), 2);
        assert_eq!(tree.get(&99), None);
    }

    #[test]
    fn clone() {
        let mut tree = RBTree::new();
        tree.insert(50);
        tree.insert(20);
        tree.insert(60);
        tree.insert(30);
        tree.insert(40);
        tree.insert(70);
        tree.insert(80);
        let tree_bis = tree.clone();

        assert_eq!(
            tree.values().collect::<Vec<i32>>(),
            tree_bis.values().collect::<Vec<i32>>()
        );

        tree.remove(&60);
        assert_eq!(tree.iter().count(), tree_bis.iter().count() - 1);
    }

    #[test]
    fn iterator() {
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

        assert_eq!(
            tree.values().collect::<Vec<i32>>(),
            vec![2, 6, 7, 8, 10, 11, 13, 18, 22, 26]
        );
    }
}
