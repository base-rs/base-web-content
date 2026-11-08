// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{ContentError, Violation};
use std::fmt;

/// Content that was refused, with every handle the caller gave the parser, so that provisional uploads can be removed.
#[non_exhaustive]
pub struct Rejected<H> {
    /// Why the content was refused.
    pub reason: Rejection,
    /// The handle of every upload the caller stored, including those a later upload replaced.
    pub handles: Vec<H>,
}

/// Shows the reason and the number of handles, since a handle may name what was submitted.
impl<H> fmt::Debug for Rejected<H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Rejected")
            .field("reason", &self.reason)
            .field("handles", &self.handles.len())
            .finish()
    }
}

/// Why content was refused: the two kinds of failure `design/errors.md` defines. It is exhaustive, so that an application can match it without a wildcard; adding a kind is a breaking change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// Parsing stopped with a fatal error.
    Error(ContentError),
    /// The body parsed, but some declared fields could not be accepted: every one, in declaration order.
    Violations(Vec<Violation>),
}

impl fmt::Display for Rejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error(error) => error.fmt(formatter),
            Self::Violations(violations) => {
                let violations: Vec<String> = violations.iter().map(Violation::to_string).collect();
                write!(
                    formatter,
                    "content has invalid fields: {}",
                    violations.join(", ")
                )
            }
        }
    }
}
