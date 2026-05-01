use std::sync::atomic::{AtomicU64, Ordering};

/// Thread-safe auto-incrementing ID generator for JSON-RPC requests
pub struct IdGenerator {
    counter: AtomicU64,
}

impl IdGenerator {
    /// Create a new ID generator starting at the given value
    pub fn new(start: u64) -> Self {
        Self {
            counter: AtomicU64::new(start),
        }
    }

    /// Get the next ID (increments and returns the new value)
    pub fn next(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }

    /// Get the current counter value without incrementing
    pub fn current(&self) -> u64 {
        self.counter.load(Ordering::SeqCst)
    }

    /// Reset the counter to a specific value
    pub fn reset(&self, value: u64) {
        self.counter.store(value, Ordering::SeqCst);
    }
}

impl Default for IdGenerator {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_new_generator_starts_at_value() {
        let id_gen = IdGenerator::new(10);
        assert_eq!(id_gen.current(), 10);
    }

    #[test]
    fn test_next_increments() {
        let id_gen = IdGenerator::new(0);
        assert_eq!(id_gen.next(), 0);
        assert_eq!(id_gen.next(), 1);
        assert_eq!(id_gen.next(), 2);
        assert_eq!(id_gen.current(), 3);
    }

    #[test]
    fn test_current_does_not_increment() {
        let id_gen = IdGenerator::new(100);
        assert_eq!(id_gen.current(), 100);
        assert_eq!(id_gen.current(), 100);
        assert_eq!(id_gen.current(), 100);
    }

    #[test]
    fn test_reset() {
        let id_gen = IdGenerator::new(0);
        id_gen.next();
        id_gen.next();
        id_gen.next();
        assert_eq!(id_gen.current(), 3);
        id_gen.reset(50);
        assert_eq!(id_gen.current(), 50);
        assert_eq!(id_gen.next(), 50);
        assert_eq!(id_gen.current(), 51);
    }

    #[test]
    fn test_default_starts_at_zero() {
        let id_gen = IdGenerator::default();
        assert_eq!(id_gen.current(), 0);
    }

    #[test]
    fn test_thread_safety() {
        let id_gen = Arc::new(IdGenerator::new(0));
        let mut handles = vec![];

        let num_threads = 8;
        let increments_per_thread = 1000;

        for _ in 0..num_threads {
            let id_gen_clone = Arc::clone(&id_gen);
            handles.push(thread::spawn(move || {
                let mut ids = vec![];
                for _ in 0..increments_per_thread {
                    ids.push(id_gen_clone.next());
                }
                ids
            }));
        }

        let mut all_ids: Vec<u64> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();

        // All IDs should be unique
        all_ids.sort();
        all_ids.dedup();
        assert_eq!(
            all_ids.len(),
            (num_threads * increments_per_thread) as usize
        );

        // Counter should be at total increments
        assert_eq!(
            id_gen.current(),
            (num_threads * increments_per_thread) as u64
        );
    }
}
