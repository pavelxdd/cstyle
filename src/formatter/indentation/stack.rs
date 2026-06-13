// Stack with O(1) clone: clones share the list and never copy elements.
// Snapshots taken per preprocessor conditional stay cheap at any depth.

use std::rc::Rc;

#[derive(Debug)]
pub struct PersistentStack<T> {
    top: Option<Rc<Node<T>>>,
    len: usize,
}

#[derive(Debug)]
struct Node<T> {
    value: T,
    next: Option<Rc<Node<T>>>,
}

impl<T> Default for PersistentStack<T> {
    fn default() -> Self {
        Self { top: None, len: 0 }
    }
}

impl<T> Clone for PersistentStack<T> {
    fn clone(&self) -> Self {
        Self {
            top: self.top.clone(),
            len: self.len,
        }
    }
}

impl<T> PersistentStack<T> {
    pub fn push(&mut self, value: T) {
        self.top = Some(Rc::new(Node {
            value,
            next: self.top.take(),
        }));
        self.len += 1;
    }

    pub fn last(&self) -> Option<&T> {
        self.top.as_deref().map(|node| &node.value)
    }
}

impl<T: PartialEq> PartialEq for PersistentStack<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        let mut a = self.top.as_deref();
        let mut b = other.top.as_deref();
        while let (Some(left), Some(right)) = (a, b) {
            if std::ptr::eq(left, right) {
                return true;
            }
            if left.value != right.value {
                return false;
            }
            a = left.next.as_deref();
            b = right.next.as_deref();
        }
        a.is_none() && b.is_none()
    }
}

impl<T: Eq> Eq for PersistentStack<T> {}

impl<T: Clone> PersistentStack<T> {
    pub fn pop(&mut self) -> Option<T> {
        let node = self.top.take()?;
        self.len -= 1;
        match Rc::try_unwrap(node) {
            Ok(node) => {
                self.top = node.next;
                Some(node.value)
            }
            Err(node) => {
                self.top = node.next.clone();
                Some(node.value.clone())
            }
        }
    }
}

// Unwind iteratively: dropping a deep owned chain node by node would
// otherwise recurse once per element and overflow the call stack.
impl<T> Drop for PersistentStack<T> {
    fn drop(&mut self) {
        let mut node = self.top.take();
        while let Some(rc) = node {
            match Rc::try_unwrap(rc) {
                Ok(mut inner) => node = inner.next.take(),
                Err(_) => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PersistentStack;

    #[test]
    fn cloned_snapshots_diverge_without_mutating_each_other() {
        let mut original = PersistentStack::default();
        original.push("base");
        original.push("original");
        let mut snapshot = original.clone();

        assert_eq!(original, snapshot);
        assert_eq!(snapshot.pop(), Some("original"));
        snapshot.push("snapshot");

        assert_eq!(original.last(), Some(&"original"));
        assert_eq!(snapshot.last(), Some(&"snapshot"));
        assert_eq!(original.len, 2);
        assert_eq!(snapshot.len, 2);
        assert_ne!(original, snapshot);
    }

    #[test]
    fn empty_and_shared_pops_keep_lengths_consistent() {
        let mut stack = PersistentStack::default();
        assert_eq!(stack.pop(), None);
        assert_eq!(stack.len, 0);

        stack.push(String::from("base"));
        let mut snapshot = stack.clone();
        assert_eq!(snapshot.pop().as_deref(), Some("base"));
        assert_eq!(snapshot.pop(), None);
        assert_eq!(snapshot.len, 0);
        assert_eq!(stack.last().map(String::as_str), Some("base"));
        assert_eq!(stack.len, 1);
    }
}
