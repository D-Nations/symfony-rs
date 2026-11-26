//! Tests/Compiler/ExtensionCompilerPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. `DummyExtension` and `CompilerPassExtension` are Rust
//! extensions; the second is also a pass (`as_compiler_pass()`).
//!
//! Pending: testExtensionAlsoRegisteredAsCompilerPassIsProcessedOnceAtItsOwnPriority
//! (it calls `compile()`, which stops at the first pass that is not
//! ported yet).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ExtensionCompilerPassTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::disallowed_names,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    clippy::unnecessary_literal_bound
)]

use std::sync::Arc;

use php_value::PhpArray;
use symfony_dependency_injection::compiler::{CompilerPassInterface, ExtensionCompilerPass, Pass};
use symfony_dependency_injection::extension::ExtensionInterface;
use symfony_dependency_injection::{ContainerBuilder, Result};

/// `DummyExtension::process()`, as a pass.
struct DummyPass(String);

impl CompilerPassInterface for DummyPass {
    fn class(&self) -> &str {
        "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CompilerPassExtension"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        container.register(&self.0, None)?;
        Ok(())
    }
}

/// `DummyExtension`, and `CompilerPassExtension` when `pass` is set.
struct DummyExtension {
    alias: String,
    pass: Option<Pass>,
}

impl DummyExtension {
    fn dummy(alias: &str) -> Self {
        Self {
            alias: alias.to_string(),
            pass: None,
        }
    }

    fn compiler_pass(alias: &str) -> Self {
        Self {
            alias: alias.to_string(),
            pass: Some(Pass::new(DummyPass(alias.to_string()))),
        }
    }
}

impl ExtensionInterface for DummyExtension {
    fn class(&self) -> &str {
        if self.pass.is_some() {
            "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\CompilerPassExtension"
        } else {
            "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\DummyExtension"
        }
    }

    fn load(&self, _configs: &[PhpArray], _container: &mut ContainerBuilder) -> Result<()> {
        Ok(())
    }

    fn get_alias(&self) -> String {
        self.alias.clone()
    }

    fn is_extension(&self) -> bool {
        true
    }

    fn as_compiler_pass(&self) -> Option<Pass> {
        self.pass.clone()
    }
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    let mut pass = ExtensionCompilerPass;

    container.register_extension(Arc::new(DummyExtension::compiler_pass("extension1")));
    container.register_extension(Arc::new(DummyExtension::dummy("extension2")));
    container.register_extension(Arc::new(DummyExtension::dummy("extension3")));
    container.register_extension(Arc::new(DummyExtension::compiler_pass("extension4")));

    pass.process(&mut container).unwrap();

    assert!(container.has_definition("extension1"));
    assert!(!container.has_definition("extension2"));
    assert!(!container.has_definition("extension3"));
    assert!(container.has_definition("extension4"));
}
