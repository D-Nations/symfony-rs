//! Port of Symfony\Component\CssSelector\CssSelectorConverter.php.
//!
//! Upstream's caches and `$maxCachedItems` are static properties, which
//! PHP keeps for one request; here they are per thread, which no caller
//! can tell apart (each entry is a pure function of its key).
//!
//! Upstream: symfony/css-selector CssSelectorConverter.php

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};

use crate::exception::Exception;
use crate::parser::shortcut::{ClassParser, ElementParser, EmptyStringParser, HashParser};
use crate::php::cat;
use crate::xpath::extension::HtmlExtension;
use crate::xpath::{DEFAULT_PREFIX, Translator, TranslatorInterface};

/// An insertion-ordered map, as the PHP array the cache is.
#[derive(Default)]
struct Cache {
    entries: HashMap<Vec<u8>, (u64, Vec<u8>)>,
    order: BTreeMap<u64, Vec<u8>>,
    next: u64,
}

impl Cache {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.entries.get(key).map(|(_, v)| v.clone())
    }

    fn remove(&mut self, key: &[u8]) {
        if let Some((seq, _)) = self.entries.remove(key) {
            self.order.remove(&seq);
        }
    }

    /// Appends (as `$cache[$key] = $value` after an `unset()`).
    fn push(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.remove(&key);
        let seq = self.next;
        self.next += 1;
        self.order.insert(seq, key.clone());
        self.entries.insert(key, (seq, value));
    }

    fn remove_first(&mut self) {
        if let Some((_, key)) = self.order.pop_first() {
            self.entries.remove(&key);
        }
    }

    fn keys(&self) -> Vec<Vec<u8>> {
        self.order.values().cloned().collect()
    }
}

thread_local! {
    static MAX_CACHED_ITEMS: Cell<usize> = const { Cell::new(1024) };
    static HTML_CACHE: RefCell<Cache> = RefCell::new(Cache::default());
    static XML_CACHE: RefCell<Cache> = RefCell::new(Cache::default());
}

/// The main entry point: CSS selectors to XPath expressions.
pub struct CssSelectorConverter {
    translator: Translator,
    html: bool,
}

impl Default for CssSelectorConverter {
    fn default() -> Self {
        Self::new(true)
    }
}

impl CssSelectorConverter {
    /// `$html`: whether HTML support is enabled (disable it for XML).
    pub fn new(html: bool) -> Self {
        let mut translator = Translator::default();
        if html {
            // The node extension is always registered by Translator::new.
            if let Ok(extension) = HtmlExtension::new(&translator) {
                translator.register_extension(extension);
            }
        }
        translator
            .register_parser_shortcut(EmptyStringParser)
            .register_parser_shortcut(ElementParser)
            .register_parser_shortcut(ClassParser)
            .register_parser_shortcut(HashParser);
        Self { translator, html }
    }

    /// `CssSelectorConverter::$maxCachedItems`.
    pub fn max_cached_items() -> usize {
        MAX_CACHED_ITEMS.with(Cell::get)
    }

    pub fn set_max_cached_items(max: usize) {
        MAX_CACHED_ITEMS.with(|m| m.set(max));
    }

    /// The cache keys in order, oldest first (upstream's tests read the
    /// static property by reflection).
    pub fn cache_keys(html: bool) -> Vec<Vec<u8>> {
        Self::with_cache(html, |cache| cache.keys())
    }

    /// Empties a cache (likewise set by reflection upstream).
    pub fn clear_cache(html: bool) {
        Self::with_cache(html, |cache| *cache = Cache::default());
    }

    fn with_cache<T>(html: bool, f: impl FnOnce(&mut Cache) -> T) -> T {
        let cache = if html { &HTML_CACHE } else { &XML_CACHE };
        cache.with(|c| f(&mut c.borrow_mut()))
    }

    /// Translates a CSS expression to XPath, with the default prefix
    /// `descendant-or-self::`.
    pub fn to_xpath(&self, css_expr: &[u8]) -> Result<Vec<u8>, Exception> {
        self.to_xpath_with_prefix(css_expr, DEFAULT_PREFIX)
    }

    pub fn to_xpath_with_prefix(
        &self,
        css_expr: &[u8],
        prefix: &[u8],
    ) -> Result<Vec<u8>, Exception> {
        let key = cat(&[prefix, b"\0", css_expr]);
        let hit = Self::with_cache(self.html, |cache| {
            let value = cache.get(&key)?;
            // Move the item last in cache (LRU)
            cache.push(key.clone(), value.clone());
            Some(value)
        });
        if let Some(value) = hit {
            return Ok(value);
        }
        Self::with_cache(self.html, |cache| {
            if cache.entries.len() >= Self::max_cached_items() {
                // Evict the oldest entry
                cache.remove_first();
            }
        });
        let value = self.translator.css_to_xpath(css_expr, prefix)?;
        Self::with_cache(self.html, |cache| cache.push(key, value.clone()));
        Ok(value)
    }
}
