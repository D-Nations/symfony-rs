//! Tests/Compiler/ExtensionCompilerPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. `DummyExtension` and `CompilerPassExtension` are Rust
//! extensions; the second is also a pass (`as_compiler_pass()`), as is
//! the anonymous extension of
//! testExtensionAlsoRegisteredAsCompilerPassIsProcessedOnceAtItsOwnPriority,
//! whose `compile()` is `support::compile()` (nothing is autowired).
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

mod support;

use std::sync::Arc;

use php_value::{PhpArray, Value};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, ExtensionCompilerPass, Pass, PassType,
};
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

/// The anonymous extension's `process()`.
struct CountingPass;

impl CompilerPassInterface for CountingPass {
    fn class(&self) -> &str {
        "Symfony\\Component\\DependencyInjection\\Extension\\Extension@anonymous"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let processed = if container.has_parameter("test.processed") {
            php_value::ops::to_int(&container.get_parameter("test.processed")?) + 1
        } else {
            1
        };
        container.set_parameter("test.processed", Value::Int(processed))?;
        let earlier = container.has_parameter("test.earlier_pass");
        container.set_parameter("test.processed_after_earlier_pass", Value::Bool(earlier))?;
        Ok(())
    }
}

/// The anonymous `Extension` that is also a `CompilerPassInterface`.
struct PassExtension(Pass);

impl ExtensionInterface for PassExtension {
    fn class(&self) -> &str {
        "Symfony\\Component\\DependencyInjection\\Extension\\Extension@anonymous"
    }

    fn load(&self, _configs: &[PhpArray], _container: &mut ContainerBuilder) -> Result<()> {
        Ok(())
    }

    fn get_alias(&self) -> String {
        "pass_extension".to_string()
    }

    fn is_extension(&self) -> bool {
        true
    }

    fn as_compiler_pass(&self) -> Option<Pass> {
        Some(self.0.clone())
    }
}

/// The anonymous pass that runs before the extension.
struct EarlierPass;

impl CompilerPassInterface for EarlierPass {
    fn class(&self) -> &str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\CompilerPassInterface@anonymous"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        container.set_parameter("test.earlier_pass", Value::Bool(true))
    }
}

#[test]
fn testExtensionAlsoRegisteredAsCompilerPassIsProcessedOnceAtItsOwnPriority() {
    let mut container = ContainerBuilder::new();
    let pass = Pass::new(CountingPass);
    let extension = Arc::new(PassExtension(pass.clone()));

    container.register_extension(extension);
    container
        .add_compiler_pass(pass, PassType::BeforeOptimization, -10000)
        .unwrap();
    container
        .add_compiler_pass(Pass::new(EarlierPass), PassType::BeforeOptimization, -5000)
        .unwrap();

    support::compile(&mut container, false).unwrap();

    assert!(
        Value::Int(1).identical(&container.get_parameter("test.processed").unwrap())
    );
    assert!(
        Value::Bool(true).identical(
            &container
                .get_parameter("test.processed_after_earlier_pass")
                .unwrap()
        )
    );
}
