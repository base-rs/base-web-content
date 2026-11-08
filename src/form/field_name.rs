// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::ContentErrorKind;

/// The components of a submitted name: its base, then each `[key]`'s key, with `[]` as an empty key, as `protocol-url`'s `Encoding.split` reads them.
///
/// The name must be `base`, `base[key]`, or `base[]`, repeated, where `base` and each `key` are non-empty and contain no brackets; any other name is not one a declaration accepts. A name with more components than `depth_limit` is refused for its depth as soon as the extra component is read.
pub(super) fn split(name: &str, depth_limit: usize) -> Result<Vec<&str>, ContentErrorKind> {
    let base_length = name.find('[').unwrap_or(name.len());
    let (base, mut rest) = name.split_at(base_length);
    if base.is_empty() || base.contains(']') {
        return Err(ContentErrorKind::UnknownName);
    }
    let mut components = Vec::new();
    let mut component = base;
    loop {
        components.push(component);
        if components.len() > depth_limit {
            return Err(ContentErrorKind::DepthLimit);
        }
        if rest.is_empty() {
            return Ok(components);
        }
        let Some((key, tail)) = rest.strip_prefix('[').and_then(|rest| rest.split_once(']')) else {
            return Err(ContentErrorKind::UnknownName);
        };
        if key.contains('[') {
            return Err(ContentErrorKind::UnknownName);
        }
        component = key;
        rest = tail;
    }
}
