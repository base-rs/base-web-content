// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::fmt;

/// Content that parsed and satisfied its declaration.
#[non_exhaustive]
pub struct Accepted<T, H> {
    /// The declared struct.
    pub content: T,
    /// The handles of uploads that a later upload for the same field replaced, which the caller should remove.
    pub discarded: Vec<H>,
}

/// Shows the number of discarded handles only, since the content holds submitted values.
impl<T, H> fmt::Debug for Accepted<T, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Accepted")
            .field("discarded", &self.discarded.len())
            .finish_non_exhaustive()
    }
}
