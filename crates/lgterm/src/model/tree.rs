//! A split tree: leaves laid out in rows or columns, nested to any depth.

use serde::{Deserialize, Serialize};

/// How a branch lays out its children.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    /// Side by side, left to right.
    Row,
    /// Stacked, top to bottom.
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// A leaf with a stable id.
pub trait Keyed {
    type Key: Copy + Eq;
    fn key(&self) -> Self::Key;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Tree<T> {
    Leaf(T),
    Branch { axis: Axis, children: Vec<Node<T>> },
}

/// A child of a branch with its share of the branch's length.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node<T> {
    pub weight: f32,
    pub tree: Tree<T>,
}

/// A rectangle in the unit square.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const UNIT: Rect = Rect {
        x: 0.,
        y: 0.,
        w: 1.,
        h: 1.,
    };
}

impl<T: Keyed> Tree<T> {
    pub fn contains(&self, key: T::Key) -> bool {
        self.find(key).is_some()
    }

    pub fn find(&self, key: T::Key) -> Option<&T> {
        match self {
            Tree::Leaf(leaf) => (leaf.key() == key).then_some(leaf),
            Tree::Branch { children, .. } => children.iter().find_map(|c| c.tree.find(key)),
        }
    }

    pub fn find_mut(&mut self, key: T::Key) -> Option<&mut T> {
        match self {
            Tree::Leaf(leaf) => (leaf.key() == key).then_some(leaf),
            Tree::Branch { children, .. } => children.iter_mut().find_map(|c| c.tree.find_mut(key)),
        }
    }

    /// Leaves in reading order.
    pub fn leaves(&self) -> Vec<&T> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect<'a>(&'a self, out: &mut Vec<&'a T>) {
        match self {
            Tree::Leaf(leaf) => out.push(leaf),
            Tree::Branch { children, .. } => children.iter().for_each(|c| c.tree.collect(out)),
        }
    }

    /// Whether every branch has at least two children, each with a finite positive weight.
    pub fn well_formed(&self) -> bool {
        match self {
            Tree::Leaf(_) => true,
            Tree::Branch { children, .. } => {
                children.len() >= 2
                    && children
                        .iter()
                        .all(|c| c.weight.is_finite() && c.weight > 0. && c.tree.well_formed())
            }
        }
    }

    pub fn first(&self) -> &T {
        match self {
            Tree::Leaf(leaf) => leaf,
            Tree::Branch { children, .. } => children[0].tree.first(),
        }
    }

    /// Put `new` after the leaf `at`, splitting it along `axis`. A branch that already runs
    /// along `axis` takes `new` as another child, sharing the old leaf's weight.
    pub fn insert(&mut self, at: T::Key, new: T, axis: Axis) -> Result<(), T> {
        self.insert_beside(at, new, axis, false)
    }

    /// Put `new` before or after the leaf `at`, splitting it along `axis` with half its share.
    pub fn insert_beside(&mut self, at: T::Key, new: T, axis: Axis, before: bool) -> Result<(), T> {
        match self {
            Tree::Leaf(leaf) if leaf.key() == at => {
                self.wrap(new, axis, before);
                Ok(())
            }
            Tree::Leaf(_) => Err(new),
            Tree::Branch {
                axis: own,
                children,
            } => {
                let own = *own;
                let Some(i) = children.iter().position(|c| c.tree.contains(at)) else {
                    return Err(new);
                };
                if own == axis && matches!(children[i].tree, Tree::Leaf(_)) {
                    let half = children[i].weight / 2.;
                    children[i].weight = half;
                    children.insert(
                        if before { i } else { i + 1 },
                        Node {
                            weight: half,
                            tree: Tree::Leaf(new),
                        },
                    );
                    Ok(())
                } else {
                    children[i].tree.insert_beside(at, new, axis, before)
                }
            }
        }
    }

    /// Put `new` before or after the whole tree along `axis`, each taking half.
    pub fn wrap(&mut self, new: T, axis: Axis, before: bool) {
        let old = std::mem::replace(
            self,
            Tree::Branch {
                axis,
                children: vec![],
            },
        );
        let mut children = vec![
            Node {
                weight: 1.,
                tree: old,
            },
            Node {
                weight: 1.,
                tree: Tree::Leaf(new),
            },
        ];
        if before {
            children.reverse();
        }
        *self = Tree::Branch { axis, children };
    }

    /// Take the leaf `key` out, giving its share to a neighbor and folding branches left with
    /// one child. The root leaf can't be removed: its owner drops the whole tree instead.
    pub fn remove(&mut self, key: T::Key) -> Option<T> {
        let Tree::Branch { children, .. } = self else {
            return None;
        };
        let i = children.iter().position(|c| c.tree.contains(key))?;
        let removed = match &children[i].tree {
            Tree::Leaf(_) => {
                let node = children.remove(i);
                let heir = i.min(children.len() - 1);
                children[heir].weight += node.weight;
                match node.tree {
                    Tree::Leaf(leaf) => leaf,
                    Tree::Branch { .. } => unreachable!(),
                }
            }
            Tree::Branch { .. } => children[i].tree.remove(key)?,
        };
        if children.len() == 1 {
            let only = children.pop().expect("one child");
            *self = only.tree;
        }
        Some(removed)
    }

    /// Each leaf's rectangle when the tree fills `area`.
    pub fn rects(&self, area: Rect) -> Vec<(T::Key, Rect)> {
        let mut out = Vec::new();
        self.place(area, &mut out);
        out
    }

    fn place(&self, area: Rect, out: &mut Vec<(T::Key, Rect)>) {
        match self {
            Tree::Leaf(leaf) => out.push((leaf.key(), area)),
            Tree::Branch { axis, children } => {
                let total: f32 = children.iter().map(|c| c.weight).sum();
                let mut at = 0.;
                for child in children {
                    let share = child.weight / total;
                    let rect = match axis {
                        Axis::Row => Rect {
                            x: area.x + at * area.w,
                            w: share * area.w,
                            ..area
                        },
                        Axis::Column => Rect {
                            y: area.y + at * area.h,
                            h: share * area.h,
                            ..area
                        },
                    };
                    child.tree.place(rect, out);
                    at += share;
                }
            }
        }
    }

    /// The branch at `path` (child indexes from the root), if there is one.
    pub fn branch_mut(&mut self, path: &[usize]) -> Option<&mut Vec<Node<T>>> {
        match (self, path.split_first()) {
            (Tree::Branch { children, .. }, None) => Some(children),
            (Tree::Branch { children, .. }, Some((&i, rest))) => {
                children.get_mut(i)?.tree.branch_mut(rest)
            }
            (Tree::Leaf(_), _) => None,
        }
    }
}

/// Moves the boundary after child `index` to `at` (0 to 1 along the branch), trading length
/// between the two children beside it. Neither ends up smaller than `min`.
pub fn move_boundary<T>(children: &mut [Node<T>], index: usize, at: f32, min: f32) {
    if index + 1 >= children.len() {
        return;
    }
    let total: f32 = children.iter().map(|c| c.weight).sum();
    let before: f32 = children[..index].iter().map(|c| c.weight).sum::<f32>() / total;
    let pair = (children[index].weight + children[index + 1].weight) / total;
    let min = min.min(pair / 2.);
    let first = (at - before).clamp(min, pair - min);
    children[index].weight = first * total;
    children[index + 1].weight = (pair - first) * total;
}

/// The rectangle beside `from` in `dir`: of those touching that edge, the one that shares the
/// most of it.
pub fn neighbor<K: Copy + Eq>(rects: &[(K, Rect)], from: K, dir: Dir) -> Option<K> {
    const EPS: f32 = 1e-4;
    let (_, a) = rects.iter().find(|(k, _)| *k == from)?;
    let overlap = |lo1: f32, hi1: f32, lo2: f32, hi2: f32| hi1.min(hi2) - lo1.max(lo2);
    rects
        .iter()
        .filter(|(k, _)| *k != from)
        .filter_map(|(k, b)| {
            let (gap, shared) = match dir {
                Dir::Left => (a.x - (b.x + b.w), overlap(a.y, a.y + a.h, b.y, b.y + b.h)),
                Dir::Right => (b.x - (a.x + a.w), overlap(a.y, a.y + a.h, b.y, b.y + b.h)),
                Dir::Up => (a.y - (b.y + b.h), overlap(a.x, a.x + a.w, b.x, b.x + b.w)),
                Dir::Down => (b.y - (a.y + a.h), overlap(a.x, a.x + a.w, b.x, b.x + b.w)),
            };
            (gap > -EPS && shared > EPS).then_some((*k, gap, shared))
        })
        .min_by(|x, y| {
            x.1.partial_cmp(&y.1)
                .unwrap()
                .then(y.2.partial_cmp(&x.2).unwrap())
        })
        .map(|(k, ..)| k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct L(u64);
    impl Keyed for L {
        type Key = u64;
        fn key(&self) -> u64 {
            self.0
        }
    }

    fn keys(t: &Tree<L>) -> Vec<u64> {
        t.leaves().iter().map(|l| l.0).collect()
    }

    #[test]
    fn insert_along_same_axis_adds_a_sibling_and_halves_the_weight() {
        let mut t = Tree::Leaf(L(1));
        t.insert(1, L(2), Axis::Row).unwrap();
        t.insert(1, L(3), Axis::Row).unwrap();
        assert_eq!(keys(&t), [1, 3, 2]);
        let rects = t.rects(Rect::UNIT);
        assert_eq!(rects[0].1.w, 0.25);
        assert_eq!(rects[1].1.w, 0.25);
        assert_eq!(rects[2].1.w, 0.5);
    }

    #[test]
    fn insert_across_axis_nests_a_branch() {
        let mut t = Tree::Leaf(L(1));
        t.insert(1, L(2), Axis::Row).unwrap();
        t.insert(2, L(3), Axis::Column).unwrap();
        let rects = t.rects(Rect::UNIT);
        assert_eq!(
            rects[2],
            (
                3,
                Rect {
                    x: 0.5,
                    y: 0.5,
                    w: 0.5,
                    h: 0.5
                }
            )
        );
        assert!(t.insert(9, L(4), Axis::Row).is_err());
    }

    #[test]
    fn remove_folds_single_child_branches_and_keeps_the_root_leaf() {
        let mut t = Tree::Leaf(L(1));
        assert_eq!(t.remove(1), None);
        t.insert(1, L(2), Axis::Row).unwrap();
        t.insert(2, L(3), Axis::Column).unwrap();
        assert_eq!(t.remove(3), Some(L(3)));
        assert_eq!(
            t.rects(Rect::UNIT)[1],
            (
                2,
                Rect {
                    x: 0.5,
                    y: 0.,
                    w: 0.5,
                    h: 1.
                }
            )
        );
        assert_eq!(t.remove(1), Some(L(1)));
        assert!(matches!(t, Tree::Leaf(L(2))));
    }

    #[test]
    fn removed_weight_goes_to_the_next_sibling() {
        let mut t = Tree::Leaf(L(1));
        t.insert(1, L(2), Axis::Row).unwrap();
        t.insert(2, L(3), Axis::Row).unwrap();
        t.remove(2);
        let rects = t.rects(Rect::UNIT);
        assert_eq!(rects[1].1.w, 0.5);
    }

    #[test]
    fn moving_a_boundary_trades_length_between_its_two_children() {
        let mut t = Tree::Leaf(L(1));
        t.insert(1, L(2), Axis::Row).unwrap();
        t.insert(2, L(3), Axis::Row).unwrap();
        let children = t.branch_mut(&[]).unwrap();
        move_boundary(children, 1, 0.6, 0.05);
        let widths: Vec<f32> = t.rects(Rect::UNIT).iter().map(|(_, r)| r.w).collect();
        assert!((widths[0] - 0.5).abs() < 1e-6);
        assert!((widths[1] - 0.1).abs() < 1e-6);
        assert!((widths[2] - 0.4).abs() < 1e-6);
        move_boundary(t.branch_mut(&[]).unwrap(), 0, 0., 0.05);
        let widths: Vec<f32> = t.rects(Rect::UNIT).iter().map(|(_, r)| r.w).collect();
        assert!((widths[0] - 0.05).abs() < 1e-6);
    }

    #[test]
    fn neighbor_prefers_the_closest_then_the_most_shared_edge() {
        // 1 | 2
        //   | 3
        let mut t = Tree::Leaf(L(1));
        t.insert(1, L(2), Axis::Row).unwrap();
        t.insert(2, L(3), Axis::Column).unwrap();
        let r = t.rects(Rect::UNIT);
        assert_eq!(neighbor(&r, 3, Dir::Left), Some(1));
        assert_eq!(neighbor(&r, 3, Dir::Up), Some(2));
        assert_eq!(neighbor(&r, 2, Dir::Down), Some(3));
        assert_eq!(neighbor(&r, 1, Dir::Left), None);
        assert_eq!(neighbor(&r, 1, Dir::Right), Some(2));
    }
}
