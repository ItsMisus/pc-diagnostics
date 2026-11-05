use std::collections::VecDeque;

/// Fixed-capacity FIFO: pushing into a full buffer drops the oldest element.
#[derive(Debug)]
pub struct Ring<T> {
    buf: VecDeque<T>,
    capacity: usize,
}

impl<T: Clone> Ring<T> {
    /// `capacity` is clamped to at least 1.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            buf: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, item: T) {
        while self.buf.len() >= self.capacity {
            self.buf.pop_front();
        }
        self.buf.push_back(item);
    }

    /// Oldest first.
    pub fn to_vec(&self) -> Vec<T> {
        self.buf.iter().cloned().collect()
    }

    /// Newest element, if any.
    pub fn last(&self) -> Option<&T> {
        self.buf.back()
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_order_and_bounds() {
        let mut r = Ring::new(3);
        assert!(r.is_empty());
        for i in 1..=2 {
            r.push(i);
        }
        assert_eq!(r.to_vec(), vec![1, 2]);
        for i in 3..=7 {
            r.push(i);
        }
        assert_eq!(r.len(), 3);
        assert_eq!(r.capacity(), 3);
        assert_eq!(r.to_vec(), vec![5, 6, 7], "oldest dropped, oldest first");
    }

    #[test]
    fn zero_capacity_is_clamped() {
        let mut r = Ring::new(0);
        r.push(1);
        r.push(2);
        assert_eq!(r.to_vec(), vec![2]);
    }
}
