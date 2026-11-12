//! Putting a leaf along the side of the largest part of a split tree that a given leaf
//! borders: between two columns of a grid, a new leaf runs their full height.

use super::tree::{Axis, Keyed, Node, Tree};

impl<T: Keyed> Tree<T> {
    /// Whether leaf `at` lies along this tree's side on `axis`: its start when `before`.
    fn borders(&self, at: T::Key, axis: Axis, before: bool) -> bool {
        match self {
            Tree::Leaf(leaf) => leaf.key() == at,
            Tree::Branch {
                axis: own,
                children,
            } => {
                let Some(i) = children.iter().position(|c| c.tree.contains(at)) else {
                    return false;
                };
                let end = if before { 0 } else { children.len() - 1 };
                (*own != axis || i == end) && children[i].tree.borders(at, axis, before)
            }
        }
    }

    /// The path, as child indexes from the root, to the largest part of the tree whose side
    /// leaf `at` lies along.
    pub fn edge_of(&self, at: T::Key, axis: Axis, before: bool) -> Option<Vec<usize>> {
        let mut path = vec![];
        let mut node = self;
        loop {
            if node.borders(at, axis, before) {
                return Some(path);
            }
            let Tree::Branch { children, .. } = node else {
                return None;
            };
            let i = children.iter().position(|c| c.tree.contains(at))?;
            path.push(i);
            node = &children[i].tree;
        }
    }

    /// The part of the tree at `path`.
    pub fn at_path(&self, path: &[usize]) -> Option<&Tree<T>> {
        match (self, path.split_first()) {
            (_, None) => Some(self),
            (Tree::Branch { children, .. }, Some((&i, rest))) => {
                children.get(i)?.tree.at_path(rest)
            }
            (Tree::Leaf(_), Some(_)) => None,
        }
    }

    /// Puts `new` beside the part at `path` (from `edge_of`), as a sibling with an even share
    /// when its branch runs along `axis`, else wrapping it.
    pub fn insert_at_edge(&mut self, path: &[usize], new: T, axis: Axis, before: bool) {
        let even = |children: &mut Vec<Node<T>>, i: usize, new: T| {
            let weight = children.iter().map(|c| c.weight).sum::<f32>() / children.len() as f32;
            children.insert(
                i,
                Node {
                    weight,
                    tree: Tree::Leaf(new),
                },
            );
        };
        match path.split_last() {
            None => match self {
                Tree::Branch {
                    axis: own,
                    children,
                } if *own == axis => {
                    let i = if before { 0 } else { children.len() };
                    even(children, i, new);
                }
                _ => self.wrap(new, axis, before),
            },
            Some((&i, parent)) => {
                let Some(Tree::Branch {
                    axis: own,
                    children,
                }) = self.at_path_mut(parent)
                else {
                    return;
                };
                if *own == axis {
                    even(children, if before { i } else { i + 1 }, new);
                } else if let Some(child) = children.get_mut(i) {
                    child.tree.wrap(new, axis, before);
                }
            }
        }
    }

    fn at_path_mut(&mut self, path: &[usize]) -> Option<&mut Tree<T>> {
        match path.split_first() {
            None => Some(self),
            Some((&i, rest)) => match self {
                Tree::Branch { children, .. } => children.get_mut(i)?.tree.at_path_mut(rest),
                Tree::Leaf(_) => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct L(u32);
    impl Keyed for L {
        type Key = u32;
        fn key(&self) -> u32 {
            self.0
        }
    }

    fn column(keys: &[u32]) -> Node<L> {
        Node {
            weight: 1.,
            tree: Tree::Branch {
                axis: Axis::Column,
                children: keys
                    .iter()
                    .map(|&k| Node {
                        weight: 1.,
                        tree: Tree::Leaf(L(k)),
                    })
                    .collect(),
            },
        }
    }

    /// Two columns of three: 1 2 3 on the left, 4 5 6 on the right.
    fn grid() -> Tree<L> {
        Tree::Branch {
            axis: Axis::Row,
            children: vec![column(&[1, 2, 3]), column(&[4, 5, 6])],
        }
    }

    fn row_keys(t: &Tree<L>) -> Vec<Vec<u32>> {
        let Tree::Branch { children, .. } = t else {
            return vec![];
        };
        children
            .iter()
            .map(|c| c.tree.leaves().iter().map(|l| l.0).collect())
            .collect()
    }

    #[test]
    fn between_two_columns_the_new_leaf_runs_their_full_height() {
        let mut t = grid();
        let path = t.edge_of(2, Axis::Row, false).unwrap();
        assert_eq!(path, [0]);
        t.insert_at_edge(&path, L(7), Axis::Row, false);
        assert_eq!(row_keys(&t), [vec![1, 2, 3], vec![7], vec![4, 5, 6]]);
        assert!(t.well_formed());
    }

    #[test]
    fn at_the_outer_edge_the_new_leaf_spans_the_whole_tree() {
        let mut t = grid();
        let path = t.edge_of(5, Axis::Row, false).unwrap();
        assert!(path.is_empty());
        t.insert_at_edge(&path, L(7), Axis::Row, false);
        assert_eq!(row_keys(&t), [vec![1, 2, 3], vec![4, 5, 6], vec![7]]);

        let mut stack = column(&[1, 2, 3]).tree;
        let path = stack.edge_of(2, Axis::Row, false).unwrap();
        stack.insert_at_edge(&path, L(4), Axis::Row, false);
        assert_eq!(row_keys(&stack), [vec![1, 2, 3], vec![4]]);
    }

    #[test]
    fn between_rows_of_one_column_the_new_leaf_is_as_wide_as_the_column() {
        let mut t = grid();
        let path = t.edge_of(1, Axis::Column, false).unwrap();
        assert_eq!(path, [0, 0]);
        t.insert_at_edge(&path, L(7), Axis::Column, false);
        assert_eq!(row_keys(&t), [vec![1, 7, 2, 3], vec![4, 5, 6]]);
    }
}
