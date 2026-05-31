use std::cell::{Cell, UnsafeCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::mem::MaybeUninit;

#[derive(Debug)]
pub struct Cachr<K, T: ?Sized> {
    inner: UnsafeCell<HashMap<K, Box<T>>>,

    /// The generation gets incremented whenever an operation on the map happens. This is used to
    /// ensure the soundness of operations on the map inside a [Self::get_or_insert] call.
    generation: Cell<u64>,

    /// The map counts as in use whenever there is an active `&mut` to [Self::inner]. It does not
    /// count as in use when the callback for [Self::get_or_insert] is being called. To make this
    /// sound, the map entry holding the reference is put into a [MaybeUninit] that only gets used
    /// again if no operation on the map has happened during the callback. This check is done with
    /// the generation.
    in_use: Cell<bool>,
}

impl<K: Hash + Eq, T: ?Sized> Cachr<K, T> {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            inner: UnsafeCell::new(HashMap::new()),
            generation: Cell::new(0),
            in_use: Cell::new(false),
        }
    }

    fn insert_inner(&self, key: K, value: impl Into<Box<T>>) -> &T {
        assert!(!self.in_use.get());
        self.in_use.set(true);
        self.generation.set(self.generation.get().strict_add(1));
        let inner = unsafe { &mut *self.inner.get() };

        use std::collections::hash_map::Entry;
        let value_ref = match inner.entry(key) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => e.insert(value.into()),
        };

        self.in_use.set(false);

        value_ref
    }

    #[inline(always)]
    /// Does nothing if key has already been inserted.
    pub fn insert_boxed(&self, key: K, value: Box<T>) {
        self.insert_inner(key, value);
    }

    #[inline(always)]
    pub fn get_or_insert_boxed<F: FnOnce() -> Box<T>>(&self, key: K, f: F) -> &T {
        assert!(!self.in_use.get());
        self.in_use.set(true);
        let expected_generation = self.generation.get().strict_add(1);
        self.generation.set(expected_generation);
        let inner = unsafe { &mut *self.inner.get() };

        use std::collections::hash_map::Entry;
        match inner.entry(key) {
            Entry::Occupied(e) => unsafe {
                // transmute lifetimes
                let value = std::mem::transmute(&**e.get());
                self.in_use.set(false);
                value
            },
            Entry::Vacant(e) => {
                // Me may or may not use the entry again depending on whether `f` makes any
                // reentrant calls to the map. Putting it inside a `MaybeUninit` insures that
                // undefined behavior only occurs if we `assume_init` on the value after the inner
                // map has been used (either mutably or immutably) instead of causing UB instantly
                // in the inner operation.
                let entry = MaybeUninit::new(e);

                // Safety:
                // We just constructed the MaybeUninit with an initialized value above.
                let key = unsafe { entry.assume_init_ref() }.key() as *const K;

                // For the duration of the call to `f` the map is not considered in use. Instead we
                // use the generation check afterwards to figure out whether an access to the map
                // has occured.
                self.in_use.set(false);

                let value = f();

                if self.generation.get() == expected_generation {
                    self.in_use.set(true);

                    // Safety:
                    // Since the generation has not been incremented, there were no operations on
                    // the map inside the call to `f`. This means the mutable reference to the map
                    // contained in the vacant entry is still valid.
                    let value = unsafe { entry.assume_init() }.insert(value);

                    self.in_use.set(false);

                    value
                } else {
                    // Since the generation has been incremented it means that another operation has
                    // happened to the map inside the call to `f`. This would mean that using
                    // `entry` again would be undefined behavior, even if it was only a read.
                    //
                    // `insert_inner` sets an unsets `in_use` itself, so there's no need to do that
                    // here.
                    //
                    // Safety:
                    // The key has been moved into `entry`, which we don't use again in this code
                    // path.
                    self.insert_inner(unsafe { std::ptr::read(key) }, value)
                }
            }
        }
    }

    #[inline(always)]
    pub fn get(&self, key: K) -> Option<&T> {
        assert!(!self.in_use.get());
        self.in_use.set(true);

        self.generation.set(self.generation.get().strict_add(1));

        let value = unsafe { &mut *self.inner.get() }.get(&key).map(|v| &**v);

        self.in_use.set(false);

        value
    }
}

impl<K: Hash + Eq, T> Cachr<K, T> {
    #[inline(always)]
    /// Does nothing if key has already been inserted.
    pub fn insert(&self, key: K, value: T) {
        self.insert_inner(key, value);
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
            generation: Cell::new(0),
            in_use: Cell::new(false),
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
        n.insert_boxed(5, vec![1, 2, 3, 4].into());
        assert_eq!(n.get(5), Some([1, 2, 3, 4].as_ref()))
    }

    #[test]
    fn insert_boxed_2() {
        let n: Cachr<usize, [usize]> = Cachr::new();
        n.insert_boxed(5, vec![1, 2, 3, 4].into());
        n.insert_boxed(6, vec![2, 3, 4, 5].into());
        assert_eq!(n.get(6), Some([2, 3, 4, 5].as_ref()))
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
        n.insert_boxed(1, vec![1, 2, 3].into());
        assert_eq!(n.get_or_insert_boxed(1, || unreachable!()), &[1, 2, 3]);
    }

    #[test]
    fn get_or_insert_boxed_2() {
        let n: Cachr<usize, [usize]> = Cachr::new();
        n.insert_boxed(1, vec![1, 2, 3].into());
        assert_eq!(
            n.get_or_insert_boxed(2, || vec![2, 3, 4].into()),
            &[2, 3, 4]
        );
    }

    #[test]
    fn as_mut() {
        let mut n: Cachr<usize, [usize]> = Cachr::new();
        n.insert_boxed(1, vec![1, 2, 3].into());
        let h = n.as_mut();
        h.insert(2, vec![2, 3, 4].into());
        assert_eq!(&*h[&2], &[2, 3, 4]);
        assert_eq!(&*h[&1], &[1, 2, 3]);
    }

    // make sure miri likes this
    #[test]
    #[should_panic]
    fn panic() {
        let n: Cachr<usize, usize> = Cachr::new();
        n.insert(0, 1);
        n.get_or_insert_boxed(1, || panic!());
    }

    #[test]
    fn reentrant_get_or_insert() {
        let n: Cachr<usize, usize> = Cachr::new();

        assert_eq!(
            *n.get_or_insert(0, || {
                n.get_or_insert(0, || 0);

                1
            }),
            0,
        );
    }

    #[test]
    fn insert_inside_get_or_insert() {
        let n: Cachr<usize, usize> = Cachr::new();

        assert_eq!(
            *n.get_or_insert(0, || {
                n.insert(0, 0);

                1
            }),
            0,
        );
    }

    #[test]
    #[should_panic]
    fn reentrant_insert() {
        use std::rc::Rc;

        struct EvilKey {
            value: usize,
            map: Option<Rc<Cachr<EvilKey, usize>>>,
        }

        impl PartialEq for EvilKey {
            fn eq(&self, other: &Self) -> bool {
                self.value == other.value
            }
        }

        impl Eq for EvilKey {}

        impl std::hash::Hash for EvilKey {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.value.hash(state);
                if let Some(ref map) = self.map {
                    map.insert(
                        EvilKey {
                            value: 12,
                            map: None,
                        },
                        12,
                    );
                }
            }
        }

        let n: Rc<Cachr<EvilKey, usize>> = Rc::new(Cachr::new());

        n.insert(
            EvilKey {
                value: 1,
                map: Some(n.clone()),
            },
            4,
        );
    }

    #[test]
    #[should_panic]
    fn insert_in_hash_in_get_or_insert() {
        use std::rc::Rc;

        struct EvilKey {
            value: usize,
            map: Option<Rc<Cachr<EvilKey, usize>>>,
        }

        impl PartialEq for EvilKey {
            fn eq(&self, other: &Self) -> bool {
                self.value == other.value
            }
        }

        impl Eq for EvilKey {}

        impl std::hash::Hash for EvilKey {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.value.hash(state);
                if let Some(ref map) = self.map {
                    map.insert(
                        EvilKey {
                            value: 12,
                            map: None,
                        },
                        12,
                    );
                }
            }
        }

        let n: Rc<Cachr<EvilKey, usize>> = Rc::new(Cachr::new());

        n.get_or_insert(
            EvilKey {
                value: 1,
                map: Some(n.clone()),
            },
            || 12,
        );
    }
}
