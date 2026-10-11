use lasso::{RodeoReader, Spur, ThreadedRodeo};
use std::sync::Arc;

/// Thread-safe string interner backed by `lasso::ThreadedRodeo`.
/// Maps strings to 4-byte `Spur` tokens without locking on read paths.
#[derive(Debug, Clone)]
pub struct Interner {
    rodeo: Arc<ThreadedRodeo>,
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

impl Interner {
    /// Create a new empty thread-safe interner.
    pub fn new() -> Self {
        Self {
            rodeo: Arc::new(ThreadedRodeo::new()),
        }
    }

    /// Intern a string slice and return its 4-byte `Spur`.
    #[inline]
    pub fn intern(&self, s: &str) -> Spur {
        self.rodeo.get_or_intern(s)
    }

    /// Resolve a `Spur` to its string slice.
    /// Panics if the `Spur` was not allocated by this interner.
    #[inline]
    pub fn resolve<'a>(&'a self, spur: &Spur) -> &'a str {
        self.rodeo.resolve(spur)
    }

    /// Try to resolve a `Spur`, returning `None` if invalid.
    #[inline]
    pub fn try_resolve<'a>(&'a self, spur: &Spur) -> Option<&'a str> {
        self.rodeo.try_resolve(spur)
    }

    /// Returns the number of unique strings interned.
    pub fn len(&self) -> usize {
        self.rodeo.len()
    }

    /// Returns true if no strings have been interned.
    pub fn is_empty(&self) -> bool {
        self.rodeo.is_empty()
    }

    /// Freeze the interner into a read-only reader for high-throughput parallel queries.
    pub fn into_reader(self) -> Result<RodeoReader, Arc<ThreadedRodeo>> {
        Arc::try_unwrap(self.rodeo).map(ThreadedRodeo::into_reader)
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
        let words = [
            "apple",
            "banana",
            "cherry",
            "date",
            "elderberry",
            "fig",
            "grape",
        ];

        // Concurrently intern repeatedly across multiple threads
        (0..1000).into_par_iter().for_each(|i| {
            let word = words[i % words.len()];
            let spur = interner.intern(word);
            assert_eq!(interner.resolve(&spur), word);
        });

        assert_eq!(interner.len(), words.len());
    }
}
