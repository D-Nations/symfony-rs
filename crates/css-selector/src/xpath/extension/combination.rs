//! Port of Symfony\Component\CssSelector\XPath\Extension\CombinationExtension.php.
//!
//! Upstream: symfony/css-selector XPath/Extension/CombinationExtension.php

use std::any::Any;
use std::rc::Rc;

use super::{CombinationTranslator, ExtensionInterface, entry};
use crate::xpath::XPathExpr;

#[derive(Debug, Clone, Copy, Default)]
pub struct CombinationExtension;

#[expect(
    clippy::needless_pass_by_value,
    reason = "one signature for every combination translator, as upstream"
)]
impl CombinationExtension {
    pub fn translate_descendant(mut xpath: XPathExpr, combined_xpath: XPathExpr) -> XPathExpr {
        xpath.join(b"/descendant-or-self::*/", &combined_xpath, None, false);
        xpath
    }

    pub fn translate_child(mut xpath: XPathExpr, combined_xpath: XPathExpr) -> XPathExpr {
        xpath.join(b"/", &combined_xpath, None, false);
        xpath
    }

    pub fn translate_direct_adjacent(mut xpath: XPathExpr, combined_xpath: XPathExpr) -> XPathExpr {
        xpath
            .join(b"/following-sibling::", &combined_xpath, None, false)
            .add_name_test()
            .and(b"position() = 1");
        xpath
    }

    pub fn translate_indirect_adjacent(
        mut xpath: XPathExpr,
        combined_xpath: XPathExpr,
    ) -> XPathExpr {
        xpath.join(b"/following-sibling::", &combined_xpath, None, false);
        xpath
    }
}

impl ExtensionInterface for CombinationExtension {
    fn get_combination_translators(&self) -> Vec<(Vec<u8>, CombinationTranslator)> {
        vec![
            entry(" ", Rc::new(|x, c| Ok(Self::translate_descendant(x, c)))),
            entry(">", Rc::new(|x, c| Ok(Self::translate_child(x, c)))),
            entry(
                "+",
                Rc::new(|x, c| Ok(Self::translate_direct_adjacent(x, c))),
            ),
            entry(
                "~",
                Rc::new(|x, c| Ok(Self::translate_indirect_adjacent(x, c))),
            ),
        ]
    }

    fn get_name(&self) -> String {
        "combination".to_owned()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
