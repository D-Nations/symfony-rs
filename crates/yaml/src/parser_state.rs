//! Port of Symfony\Component\Yaml\ParserState.php: nesting depth and
//! collection alias limits for one parse.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/yaml ParserState.php

use php_value::{Value, object::StdClass};

use crate::{exception::ParseException, tag::TaggedValue};

/// `Parser::DEFAULT_MAX_NESTING_LEVEL`.
pub const DEFAULT_MAX_NESTING_LEVEL: i64 = 128;
/// `Parser::DEFAULT_MAX_ALIASES_FOR_COLLECTIONS`.
pub const DEFAULT_MAX_ALIASES_FOR_COLLECTIONS: i64 = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserState {
    pub max_nesting_level: i64,
    pub current_nesting_level: i64,
    pub max_aliases_for_collections: i64,
    pub collection_alias_count: i64,
    pub aliases_enabled: bool,
}

impl Default for ParserState {
    fn default() -> Self {
        Self {
            max_nesting_level: DEFAULT_MAX_NESTING_LEVEL,
            current_nesting_level: 0,
            max_aliases_for_collections: DEFAULT_MAX_ALIASES_FOR_COLLECTIONS,
            collection_alias_count: 0,
            aliases_enabled: true,
        }
    }
}

impl ParserState {
    pub fn reset(&mut self) {
        self.current_nesting_level = 0;
        self.collection_alias_count = 0;
        self.aliases_enabled = true;
    }

    pub fn enter_nesting_level(
        &mut self,
        line: i64,
        snippet: Option<&[u8]>,
        filename: Option<&str>,
    ) -> Result<(), ParseException> {
        self.current_nesting_level += 1;
        if self.current_nesting_level > self.max_nesting_level {
            self.current_nesting_level -= 1;
            return Err(ParseException::new(
                format!(
                    "Maximum nesting depth of {} exceeded.",
                    self.max_nesting_level
                ),
                line,
                snippet.map(Into::into),
                filename.map(str::to_owned),
            ));
        }
        Ok(())
    }

    pub fn leave_nesting_level(&mut self) {
        if self.current_nesting_level > 0 {
            self.current_nesting_level -= 1;
        }
    }

    pub fn count_alias(
        &mut self,
        ref_value: &Value,
        line: i64,
        snippet: Option<&[u8]>,
        filename: Option<&str>,
    ) -> Result<(), ParseException> {
        let error = |message: String| {
            ParseException::new(
                message,
                line,
                snippet.map(Into::into),
                filename.map(str::to_owned),
            )
        };
        if !self.aliases_enabled {
            return Err(error("Aliases are disabled.".to_owned()));
        }
        let value = match ref_value
            .as_object()
            .and_then(|o| o.downcast_ref::<TaggedValue>())
        {
            Some(tagged) => tagged.get_value(),
            None => ref_value,
        };
        let collection = matches!(value, Value::Array(_))
            || value
                .as_object()
                .is_some_and(|o| o.downcast_ref::<StdClass>().is_some());
        if !collection {
            return Ok(());
        }
        self.collection_alias_count += 1;
        if self.collection_alias_count > self.max_aliases_for_collections {
            return Err(error(format!(
                "Maximum number of collection aliases ({}) exceeded. This limit can be increased via the Parser constructor.",
                self.max_aliases_for_collections
            )));
        }
        Ok(())
    }
}
