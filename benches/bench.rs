use cachr::Cachr;
use std::collections::HashMap;

use brunch::{Bench, benches};

// Let the macro handle everything for you.
benches!(
    Bench::new("cachr insert 1000")
        .run(|| {
            let n: Cachr<usize, usize> = Cachr::new();
            for i in 0..1000 {
                n.insert(i, i);
            }

            std::hint::black_box(n);
        }),
    
    Bench::new("std insert 1000")
        .run(|| {
            let mut n: HashMap<usize, Box<usize>> = HashMap::new();
            for i in 0..1000 {
                n.insert(i, Box::new(i));
            }

            std::hint::black_box(n);
        }),

    Bench::new("elsa insert 1000")
        .run(|| {
            let mut n: elsa::FrozenMap<usize, Box<usize>> = elsa::FrozenMap::new();
            for i in 0..1000 {
                n.insert(i, Box::new(i));
            }

            std::hint::black_box(n);
        }),

    Bench::new("cachr insert/get 1000")
        .run(|| {
            let n: Cachr<usize, usize> = Cachr::new();
            for i in 0..1000 {
                std::hint::black_box(n.get_or_insert(i % 128, || i));
            }

            std::hint::black_box(n);
        }),
    
    Bench::new("std insert/get 1000")
        .run(|| {
            let mut n: HashMap<usize, Box<usize>> = HashMap::new();
            for i in 0..1000 {
                use std::collections::hash_map::Entry;
                std::hint::black_box(match n.entry(i % 128) {
                    Entry::Occupied(e) => **e.get(),
                    Entry::Vacant(e) => {
                        **e.insert(Box::new(i))
                    }
                });
            }

            std::hint::black_box(n);
        }),

    Bench::new("elsa insert/get 1000")
        .run(|| {
            let mut n: elsa::FrozenMap<usize, Box<usize>> = elsa::FrozenMap::new();
            for i in 0..1000 {
                std::hint::black_box(match n.get(&(i % 128)) {
                    Some(v) => v,
                    None => n.insert(i % 64, Box::new(i)),
                });
            }

            std::hint::black_box(n);
        }),
);
