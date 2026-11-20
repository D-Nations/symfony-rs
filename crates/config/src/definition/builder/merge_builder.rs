//! Port of Symfony\Component\Config\Definition\Builder\MergeBuilder.php:
//! whether a node can be unset (`false`) or overwritten when configs are
//! merged. Upstream reaches it through the protected `merge()`; here the
//! definitions use it directly.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/MergeBuilder.php

/// MergeBuilder's properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MergeBuilder {
    pub(crate) allow_false: bool,
    pub(crate) allow_overwrite: bool,
}

impl Default for MergeBuilder {
    fn default() -> Self {
        Self {
            allow_false: false,
            allow_overwrite: true,
        }
    }
}

impl MergeBuilder {
    /// `allowUnset($allow)`.
    pub(crate) fn allow_unset(&mut self, allow: bool) {
        self.allow_false = allow;
    }

    /// `denyOverwrite($deny)`.
    pub(crate) fn deny_overwrite(&mut self, deny: bool) {
        self.allow_overwrite = !deny;
    }
}
