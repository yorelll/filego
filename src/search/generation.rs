//! Query generation token for rejecting stale asynchronous results (M02.5).
//!
//! The pure search core does not spawn work. A presenter increments this token
//! every time the query/filter input changes, attaches the current value to a
//! background request, and discards a completed response unless it still equals
//! the active generation. The monotonically increasing counter wraps only at
//! `u64::MAX`, where zero is safely a new generation because no in-flight
//! request can legitimately be older than a full u64 cycle in practice.

/// Monotonic generation marker owned by a query presenter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QueryGeneration {
    generation: u64,
}

impl QueryGeneration {
    /// Create the initial generation (zero).
    pub const fn new() -> Self {
        Self { generation: 0 }
    }

    /// Current generation value, suitable for attaching to an asynchronous
    /// request/result.
    pub const fn current(self) -> u64 {
        self.generation
    }

    /// Advance to and return the next generation, wrapping deterministically
    /// on integer overflow (release and debug behave identically).
    pub fn next_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    /// Whether `completed_generation` is stale relative to this active token.
    pub const fn is_stale(self, completed_generation: u64) -> bool {
        completed_generation != self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::QueryGeneration;

    #[test]
    fn next_generation_marks_prior_result_stale() {
        let mut generation = QueryGeneration::new();
        assert_eq!(generation.current(), 0);
        assert!(!generation.is_stale(0));

        let first = generation.next_generation();
        assert_eq!(first, 1);
        assert!(!generation.is_stale(first));

        let second = generation.next_generation();
        assert_eq!(second, 2);
        assert!(generation.is_stale(first));
        assert!(!generation.is_stale(second));
    }
}
