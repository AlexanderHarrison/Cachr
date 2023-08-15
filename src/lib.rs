use std::collections::HashMap;
use std::cell::UnsafeCell;
use std::hash::Hash;

#[repr(transparent)]
#[derive(Debug)]
pub struct Cachr<K, T: ?Sized> {
    inner: UnsafeCell<HashMap<K, Box<T>>>,
}

impl<K: Hash + Eq, T: ?Sized> Cachr<K, T> {
    #[inline(always)]
    pub fn new() -> Self {
        Self { inner: UnsafeCell::new(HashMap::new()) }
    }

    #[inline(always)]
    pub fn insert_boxed(&self, key: K, value: Box<T>) {
        unsafe { &mut *self.inner.get() }.insert(key, value);
    }

    #[inline(always)]
    pub fn get_or_insert_boxed<F: FnOnce() -> Box<T>>(&self, key: K, f: F) -> &T {
        let inner = unsafe { &mut *self.inner.get() };

        use std::collections::hash_map::Entry;
        match inner.entry(key) {
            Entry::Occupied(e) => unsafe {
                // transmute lifetimes
                std::mem::transmute(&**e.get())
            }
            Entry::Vacant(e) => e.insert(f()),
        }
    }

    #[inline(always)]
    pub fn get(&self, key: K) -> Option<&T> {
        unsafe { &mut *self.inner.get() }.get(&key)
            .map(|v| &**v)
    }
}

impl<K: Hash + Eq, T> Cachr<K, T> {
    #[inline(always)]
    pub fn insert(&self, key: K, value: T) {
        let boxed = Box::new(value);
        self.insert_boxed(key, boxed);
    }

    #[inline(always)]
    pub fn get_or_insert<F: FnOnce() -> T>(&self, key: K, f: F) -> &T {
        self.get_or_insert_boxed(key, || Box::new(f()))
    }
}

impl<K, V: ?Sized> std::convert::AsMut<HashMap<K, Box<V>>> for Cachr<K, V> {
    fn as_mut(&mut self) -> &mut HashMap<K, Box<V>> {
        self.inner.get_mut()
    }
}

impl<K: Hash + Eq, V: ?Sized> Default for Cachr<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Hash + Eq, V: ?Sized> std::ops::Index<K> for Cachr<K, V> {
    type Output = V;

    fn index(&self, key: K) -> &Self::Output {
        self.get(key).unwrap()
    }
}

impl<K, V: ?Sized> From<HashMap<K, Box<V>>> for Cachr<K, V> {
    fn from(hashmap: HashMap<K, Box<V>>) -> Self {
        Self {
            inner: UnsafeCell::new(hashmap),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_1() {
        let n: Cachr<usize, usize> = Cachr::new();
        n.insert(1, 4);
        assert_eq!(n.get(1), Some(&4))
    }

    #[test]
    fn insert_2() {
        let n: Cachr<usize, usize> = Cachr::new();
        n.insert(1, 4);
        n.insert(2, 6);
        assert_eq!(n.get(2), Some(&6))
    }

    #[test]
    fn insert_boxed_1() {
        let n: Cachr<usize, [usize]> = Cachr::new();
        n.insert_boxed(5, vec![1,2,3,4].into());
        assert_eq!(n.get(5), Some([1,2,3,4].as_ref()))
    }

    #[test]
    fn insert_boxed_2() {
        let n: Cachr<usize, [usize]> = Cachr::new();
        n.insert_boxed(5, vec![1,2,3,4].into());
        n.insert_boxed(6, vec![2,3,4,5].into());
        assert_eq!(n.get(6), Some([2,3,4,5].as_ref()))
    }

    #[test]
    fn get_or_insert_1() {
        let n: Cachr<usize, usize> = Cachr::new();
        n.insert(1, 4);
        assert_eq!(*n.get_or_insert(1, || unreachable!()), 4);
    }

    #[test]
    fn get_or_insert_2() {
        let n: Cachr<usize, usize> = Cachr::new();
        n.insert(1, 4);
        assert_eq!(*n.get_or_insert(2, || 5), 5);
    }

    #[test]
    fn get_or_insert_boxed_1() {
        let n: Cachr<usize, [usize]> = Cachr::new();
        n.insert_boxed(1, vec![1,2,3].into());
        assert_eq!(n.get_or_insert_boxed(1, || unreachable!()), &[1,2,3]);
    }

    #[test]
    fn get_or_insert_boxed_2() {
        let n: Cachr<usize, [usize]> = Cachr::new();
        n.insert_boxed(1, vec![1,2,3].into());
        assert_eq!(n.get_or_insert_boxed(2, || vec![2,3,4].into()), &[2,3,4]);
    }

    #[test]
    fn as_mut() {
        let mut n: Cachr<usize, [usize]> = Cachr::new();
        n.insert_boxed(1, vec![1,2,3].into());
        let h = n.as_mut();
        h.insert(2, vec![2,3,4].into());
        assert_eq!(&*h[&2], &[2,3,4]);
        assert_eq!(&*h[&1], &[1,2,3]);
    }

    // make sure miri likes this
    #[test] #[should_panic]
    fn panic() { 
        let n: Cachr<usize, usize> = Cachr::new();
        n.insert(0, 1);
        n.get_or_insert_boxed(1, || panic!());
    }
}
