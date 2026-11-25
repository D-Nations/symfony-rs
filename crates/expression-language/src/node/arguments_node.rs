//! Port of Symfony\Component\ExpressionLanguage\Node\ArgumentsNode.php:
//! an [`ArrayNode`] made with [`ArrayNode::arguments`], compiled and
//! dumped as a list of values separated by commas.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Node/ArgumentsNode.php

use super::ArrayNode;
use crate::syntax_error::Error;

impl ArrayNode {
    /// `ArgumentsNode::toArray()`: the values, separated by `, `.
    pub(crate) fn dump_arguments(&self, out: &mut Vec<u8>) -> Result<(), Error> {
        for (i, value) in self.values().enumerate() {
            if i > 0 {
                out.extend_from_slice(b", ");
            }
            value.dump_into(out)?;
        }
        Ok(())
    }
}
