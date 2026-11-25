//! Port of Symfony\Component\CssSelector\XPath\Extension\RelationExtension.php.
//!
//! Upstream: symfony/css-selector XPath/Extension/RelationExtension.php

use std::any::Any;
use std::rc::Rc;

use super::{CombinationTranslator, ExtensionInterface, entry};
use crate::xpath::XPathExpr;

#[derive(Debug, Clone, Copy, Default)]
pub struct RelationExtension;

#[expect(
    clippy::needless_pass_by_value,
    reason = "one signature for every relation translator, as upstream"
)]
impl RelationExtension {
    pub fn translate_relation_descendant(
        mut xpath: XPathExpr,
        combined_xpath: XPathExpr,
    ) -> XPathExpr {
        xpath.join(b"[descendant-or-self::", &combined_xpath, Some(b"]"), true);
        xpath
    }

    pub fn translate_relation_child(mut xpath: XPathExpr, combined_xpath: XPathExpr) -> XPathExpr {
        xpath.join(b"[./", &combined_xpath, Some(b"]"), true);
        xpath
    }

    pub fn translate_relation_direct_adjacent(
        mut xpath: XPathExpr,
        mut combined_xpath: XPathExpr,
    ) -> XPathExpr {
        // position() = 1 must apply to the sibling, not the outer element,
        // so it goes on combined_xpath before the join
        combined_xpath.add_name_test().and(b"position() = 1");
        xpath.join(b"[following-sibling::", &combined_xpath, Some(b"]"), true);
        xpath
    }

    pub fn translate_relation_indirect_adjacent(
        mut xpath: XPathExpr,
        combined_xpath: XPathExpr,
    ) -> XPathExpr {
        xpath.join(b"[following-sibling::", &combined_xpath, Some(b"]"), true);
        xpath
    }
}

impl ExtensionInterface for RelationExtension {
    fn get_relative_combination_translators(&self) -> Vec<(Vec<u8>, CombinationTranslator)> {
        vec![
            entry(
                " ",
                Rc::new(|x, c| Ok(Self::translate_relation_descendant(x, c))),
            ),
            entry(
                ">",
                Rc::new(|x, c| Ok(Self::translate_relation_child(x, c))),
            ),
            entry(
                "+",
                Rc::new(|x, c| Ok(Self::translate_relation_direct_adjacent(x, c))),
            ),
            entry(
                "~",
                Rc::new(|x, c| Ok(Self::translate_relation_indirect_adjacent(x, c))),
            ),
        ]
    }

    fn get_name(&self) -> String {
        "relation".to_owned()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
