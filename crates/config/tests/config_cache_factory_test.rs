//! ConfigCacheFactoryTest.php at symfony/config v8.1.10. Its one test
//! passes a non-callable, which a Rust closure type rules out (`n/a` in
//! the manifest); this file checks what `cache()` does with a callable.
//!
//! Upstream: none
#![allow(non_snake_case)]

mod common;

use common::{TestResult, cache::set_up, fs::unlink};
use symfony_config::{
    ConfigCacheFactory, ConfigCacheFactoryInterface, ConfigCacheInterface,
    ResourceCheckerConfigCacheFactory,
};

#[test]
fn cache_calls_the_callable_only_when_stale() -> TestResult {
    let files = set_up();
    unlink(&files.cache_file);
    for factory in [
        Box::new(ConfigCacheFactory::new(true)) as Box<dyn ConfigCacheFactoryInterface>,
        Box::new(ResourceCheckerConfigCacheFactory::new([])),
    ] {
        let _ = std::fs::remove_file(common::fs::os(&files.cache_file));
        let mut calls = 0;
        let mut fill = |cache: &mut dyn ConfigCacheInterface| {
            calls += 1;
            cache.write(b"filled", Some(&[]))
        };
        let mut cache = factory.cache(&files.cache_file, &mut fill)?;
        assert!(cache.is_fresh()?);
        factory.cache(&files.cache_file, &mut fill)?;
        assert_eq!(calls, 1);
        assert_eq!(std::fs::read(common::fs::os(&files.cache_file))?, b"filled");
    }
    Ok(())
}
