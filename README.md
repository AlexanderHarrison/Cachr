# Cachr

[![Current Version](https://img.shields.io/crates/v/cachr.svg)](https://crates.io/crates/cachr)
[![License: MIT/Apache-2.0](https://img.shields.io/crates/l/cachr.svg)]()

A simple rust crate providing shared read and write access to a hashmap, by disallowing removal of entries.
This is useful for caching, where data will only ever be read or included in the cache.

Because entries can never be mutated or deleted, 
immutable references to entries will be valid for the entire lifetime of the cache. 
This means you can safely add entries to the cache 
while holding references to other entires without violating aliasing rules.
All of the entries must be stored under a layer of indirection (such as `Box`), 
so the underlying hashmap can reallocate without changing references.

```rust
fn main() {
    let cache = cachr::Cachr::new();

    let s1 = SomeStruct::new(&cache);
    let s2 = SomeOtherStruct::new(&cache);

    // s1 and s2 will both be able to read and write 
    // (but not remove) entries from the cache.

    do_something_1(&s1);
    do_something_2(&s2);

    // some_complex_function is only called once
    println!("{}", cache[0]);
}

fn do_something_1(s1: &SomeStruct) {
    s1.cache.get_or_insert(0, || some_complex_function());
}

fn do_something_2(s2: &SomeOtherStruct) {
    s2.cache.get_or_insert(0, || some_complex_function());
}
```

You can emulate this behaviour by passing in a mutable reference 
to some cache struct every time you call a function needing it, 
but this rapidly becomes tedious.

### Comparison to [elsa](https://crates.io/crates/elsa)?

Elsa is broader in features, but is more complex than Cachr.
This crate only allows boxed objects, only provides a hashmap implementation,
but does so in 80 lines of simple, tested, benchmarked code.

Cachr also provides `get_or_insert` semantics, which skips
a repeat hash lookup in the common case of needing to compute something if has not yet been cached.
Elsa does not provide this.
Other than this case, Cachr and Elsa both perform identically to standard HashMap.
