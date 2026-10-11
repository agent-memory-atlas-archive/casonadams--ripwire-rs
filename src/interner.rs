use lasso::{Spur, ThreadedRodeo};
use std::sync::Arc;

/// Thread-safe string interner backed by `lasso::ThreadedRodeo`.
/// Maps strings to 4-byte `Spur` tokens without locking on read paths.
#[derive(Debug, Clone, Default)]
pub struct Interner {
    rodeo: Arc<ThreadedRodeo>,
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn intern(&self, s: &str) -> Spur {
        self.rodeo.get_or_intern(s)
    }

    #[inline]
    pub fn resolve<'a>(&'a self, spur: &Spur) -> &'a str {
        self.rodeo.resolve(spur)
    }

    #[inline]
    pub fn try_resolve<'a>(&'a self, spur: &Spur) -> Option<&'a str> {
        self.rodeo.try_resolve(spur)
    }

    pub fn len(&self) -> usize {
        self.rodeo.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rodeo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon::prelude::*;

    #[test]
    fn test_intern_and_resolve() {
        let interner = Interner::new();
        let s1 = interner.intern("foo");
        let s2 = interner.intern("bar");
        let s3 = interner.intern("foo");

        assert_eq!(s1, s3);
        assert_ne!(s1, s2);
        assert_eq!(interner.resolve(&s1), "foo");
        assert_eq!(interner.resolve(&s2), "bar");
        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn test_multi_threaded_interning() {
        let interner = Interner::new();
        let words = ["apple", "banana", "cherry", "date", "fig"];

        (0..100).into_par_iter().for_each(|i| {
            let word = words[i % words.len()];
            let spur = interner.intern(word);
            assert_eq!(interner.resolve(&spur), word);
        });

        assert_eq!(interner.len(), words.len());
    }
}
