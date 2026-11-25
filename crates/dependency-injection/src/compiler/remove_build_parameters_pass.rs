//! Port of Symfony\Component\DependencyInjection\Compiler\RemoveBuildParametersPass.php.
//!
//! Removes the build parameters (names starting with a dot), keeping
//! array ones when asked to.
//!
//! Upstream: symfony/dependency-injection Compiler/RemoveBuildParametersPass.php

use php_value::{PhpArray, Value};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::exception::Result;

/// `RemoveBuildParametersPass`.
#[derive(Debug, Default)]
pub struct RemoveBuildParametersPass {
    removed_parameters: PhpArray,
    preserve_arrays: bool,
}

impl RemoveBuildParametersPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\RemoveBuildParametersPass";

    /// `new RemoveBuildParametersPass($preserveArrays)`.
    pub fn new(preserve_arrays: bool) -> Self {
        Self {
            removed_parameters: PhpArray::new(),
            preserve_arrays,
        }
    }

    /// `getRemovedParameters()`.
    pub fn get_removed_parameters(&self) -> &PhpArray {
        &self.removed_parameters
    }
}

impl CompilerPassInterface for RemoveBuildParametersPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        self.removed_parameters = PhpArray::new();

        let parameters = container.get_parameter_bag().all().clone();
        for (name, value) in parameters.iter() {
            let name_bytes = name.bytes();
            if name_bytes.first() != Some(&b'.') {
                continue;
            }
            let name = String::from_utf8_lossy(&name_bytes).into_owned();
            if !self.preserve_arrays || !matches!(value, Value::Array(_)) {
                self.removed_parameters.insert(name.as_str(), value.clone());
                container.get_parameter_bag_mut().remove(&name)?;
                container.log(
                    Self::CLASS,
                    &format!("Removing build parameter \"{name}\"."),
                )?;
            } else {
                container.log(
                    Self::CLASS,
                    &format!(
                        "Keeping array build parameter \"{name}\" for placeholder resolution."
                    ),
                )?;
            }
        }
        Ok(())
    }
}
