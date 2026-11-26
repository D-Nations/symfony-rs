//! Port of Symfony\Component\DependencyInjection\Compiler\MergeExtensionConfigurationPass.php:
//! merges extension configs into the container builder.
//!
//! The file's two internal subclasses have no Rust subclasses:
//!
//! - `MergeExtensionConfigurationContainerBuilder` is a `ContainerBuilder`
//!   made with [`ContainerBuilder::for_extension`], which refuses
//!   `addCompilerPass()`, `compile()` and a cast env var resolved at
//!   compile time (not `registerExtension()`, which cannot fail in this
//!   port).
//! - `MergeExtensionConfigurationParameterBag` is an env placeholder bag
//!   whose placeholders `freezeAfterProcessing()` replaces with the
//!   processed ones (and the others become its unused ones), which is
//!   what its `getEnvPlaceholders()` and `getUnusedEnvPlaceholders()`
//!   overrides return.
//!
//! When the extension is not an `Extension`, upstream hands the
//! container's own bag to the extension's container; here it is a copy,
//! whose parameters `merge()` adds back (and whose env placeholders are
//! merged back when `load()` fails), as sharing the object does. The
//! port has no `ExpressionLanguage`, so there are no providers to forward.
//!
//! Upstream: symfony/dependency-injection Compiler/MergeExtensionConfigurationPass.php

use php_value::{PhpArray, Value, serialize::serialize};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::{ContainerBuilder, EnvFormat};
use crate::exception::{ErrorKind, Result};
use crate::extension::ExtensionInterface;
use crate::parameter_bag::{EnvPlaceholderMap, ParameterBag};

/// `MergeExtensionConfigurationPass`.
#[derive(Debug, Clone, Default)]
pub struct MergeExtensionConfigurationPass {
    extensions: Vec<String>,
}

impl MergeExtensionConfigurationPass {
    /// `new MergeExtensionConfigurationPass()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `new MergeExtensionConfigurationPass($extensions)`: the aliases of
    /// the extensions to load when no configuration is given for them.
    pub fn with_extensions(extensions: Vec<String>) -> Self {
        Self { extensions }
    }
}

fn configs_value(configs: &[PhpArray]) -> Value {
    Value::Array(PhpArray::list(configs.iter().cloned().map(Value::Array)))
}

fn configs_from(value: Value) -> Vec<PhpArray> {
    match value {
        Value::Array(a) => a
            .values()
            .map(|v| match v {
                Value::Array(a) => a.clone(),
                _ => PhpArray::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

impl CompilerPassInterface for MergeExtensionConfigurationPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\MergeExtensionConfigurationPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        for extension in &self.extensions {
            if container.get_extension_config(extension).is_empty() {
                container.load_from_extension(extension, Some(PhpArray::new()))?;
            }
        }
        let parameters = container.get_parameter_bag().all().clone();
        let definitions = container.get_definitions().clone();
        let aliases = container.get_aliases().clone();

        let extensions: Vec<_> = container.get_extensions().values().cloned().collect();
        for extension in &extensions {
            if let Some(result) = extension.prepend(container) {
                result?;
            }
        }

        let extensions: Vec<(String, _)> = container
            .get_extensions()
            .iter()
            .map(|(name, e)| (name.to_string(), e.clone()))
            .collect();
        for (name, extension) in extensions {
            let config = container.get_extension_config(&name);
            if config.is_empty() {
                // this extension was not called
                continue;
            }
            let bag = container.get_parameter_bag();
            let dedicated = bag.is_env_placeholder() && extension.is_extension();
            let resolving_bag = if dedicated {
                // create a dedicated bag so that we can track env vars per-extension
                let resolving = ParameterBag::env_placeholder(bag.all())?;
                resolving.merge_env_placeholders(bag);
                symfony_config::definition::set_placeholder_unique_prefix(
                    resolving.get_env_placeholder_unique_prefix().as_bytes(),
                );
                resolving
            } else {
                bag.clone()
            };

            let config = match resolving_bag.resolve_value(&configs_value(&config), &[]) {
                Ok(config) => configs_from(config),
                Err(mut e) => {
                    if e.is(&ErrorKind::ParameterNotFound) {
                        e.update_parameter_not_found(|f| {
                            f.source_extension_name = Some(name.clone());
                        });
                    }
                    if !dedicated {
                        container
                            .get_parameter_bag()
                            .merge_env_placeholders(&resolving_bag);
                    }
                    return Err(e);
                }
            };

            let mut tmp_container =
                ContainerBuilder::for_extension(extension.class(), resolving_bag);
            tmp_container.set_class_registry(container.class_registry());
            let loaded = load(extension.as_ref(), &config, container, &mut tmp_container);
            if let Err(e) = loaded {
                container
                    .get_parameter_bag()
                    .merge_env_placeholders(tmp_container.get_parameter_bag());
                return Err(e);
            }

            if dedicated {
                // don't keep track of env vars that are *overridden* when configs are merged
                freeze_after_processing(extension.as_ref(), &tmp_container);
            }

            container.merge(&tmp_container)?;
            container.get_parameter_bag_mut().add(&parameters)?;
        }

        let current: Vec<(String, _)> = container
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        for (id, definition) in current {
            if aliases.contains_key(&id)
                || definitions.get(&id).is_some_and(|d| !d.ptr_eq(&definition))
            {
                // count the env vars of extension definitions that user-defined services or aliases replace
                container.resolve_env_placeholders(
                    definition.to_value(),
                    &EnvFormat::Placeholder,
                    None,
                )?;
            }
        }

        container.add_definitions(
            definitions
                .iter()
                .map(|(id, d)| (id.to_string(), d.clone())),
        )?;
        container.add_aliases(aliases.iter().map(|(id, a)| (id.to_string(), a.clone())))?;
        Ok(())
    }
}

/// The `try` block around `$extension->load()`.
fn load(
    extension: &dyn ExtensionInterface,
    config: &[PhpArray],
    container: &ContainerBuilder,
    tmp_container: &mut ContainerBuilder,
) -> Result<()> {
    tmp_container.set_resource_tracking(container.is_tracking_resources());
    tmp_container.add_object_resource(extension.class())?;
    if let Some(configuration) = extension.get_configuration_class(config, tmp_container) {
        tmp_container.add_object_resource(&configuration)?;
    }
    extension.load(config, tmp_container)
}

/// `$a + $b` of two placeholder maps.
fn union(a: &EnvPlaceholderMap, b: &EnvPlaceholderMap) -> EnvPlaceholderMap {
    let mut out = a.clone();
    out.union(b);
    out
}

/// `array_diff_key($a, $b)`.
fn diff_key(a: &EnvPlaceholderMap, b: &EnvPlaceholderMap) -> EnvPlaceholderMap {
    let mut out = EnvPlaceholderMap::new();
    for (k, v) in a.iter() {
        if !b.contains_key(k) {
            out.insert(k, v.clone());
        }
    }
    out
}

/// `MergeExtensionConfigurationParameterBag::freezeAfterProcessing()`.
fn freeze_after_processing(extension: &dyn ExtensionInterface, container: &ContainerBuilder) {
    let bag = container.get_parameter_bag();
    let placeholders = bag.get_env_placeholders();
    let unused = bag.get_unused_env_placeholders();
    let config = extension.get_processed_configs().unwrap_or_default();
    let (mut processed, candidates) = if config.is_empty() {
        // Extension::processConfiguration() wasn't called, we cannot know how configs were merged
        let processed = placeholders.clone();
        let candidates = diff_key(&unused, &processed);
        (processed, candidates)
    } else {
        (EnvPlaceholderMap::new(), union(&placeholders, &unused))
    };

    // serialize config and container to catch env vars nested in object graphs
    let mut definitions = PhpArray::new();
    for (id, definition) in container.get_definitions().iter() {
        definitions.insert(id, definition.to_value());
    }
    let mut aliases = PhpArray::new();
    for (id, alias) in container.get_aliases().iter() {
        aliases.insert(id, alias.to_value());
    }
    let mut serialized = serialize(&configs_value(&config)).as_bytes().to_vec();
    serialized.extend_from_slice(serialize(&Value::Array(definitions)).as_bytes());
    serialized.extend_from_slice(serialize(&Value::Array(aliases)).as_bytes());
    serialized.extend_from_slice(serialize(&Value::Array(bag.all().clone())).as_bytes());

    if !candidates.is_empty() && contains_ci(&serialized, b"env_") {
        let used = env_placeholder_matches(&serialized);
        for (env, list) in candidates.iter() {
            if list.iter().any(|p| used.contains(&p.as_bytes().to_vec())) {
                processed.insert(env, list.clone());
            }
        }
    }

    bag.set_unused_env_placeholders(diff_key(&placeholders, &processed));
    bag.set_env_placeholders(processed);
}

/// `false !== stripos($haystack, $needle)`.
fn contains_ci(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle))
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn hex_run(s: &[u8], n: usize) -> bool {
    s.len() >= n && s[..n].iter().all(u8::is_ascii_hexdigit)
}

/// `preg_match_all('/env_[a-f0-9]{16}_\w+_[a-f0-9]{32}/Ui', ...)`: the
/// matches, in order (the `U` flag makes `\w+` lazy).
fn env_placeholder_matches(subject: &[u8]) -> Vec<Vec<u8>> {
    let mut matches = Vec::new();
    let mut i = 0;
    while i + 4 <= subject.len() {
        if let Some(end) = match_at(subject, i) {
            matches.push(subject[i..end].to_vec());
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    matches
}

fn match_at(s: &[u8], start: usize) -> Option<usize> {
    let rest = &s[start..];
    if rest.len() < 4 || !rest[..4].eq_ignore_ascii_case(b"env_") || !hex_run(&rest[4..], 16) {
        return None;
    }
    let mut pos = 4 + 16;
    if rest.get(pos) != Some(&b'_') {
        return None;
    }
    pos += 1;
    // \w+ lazily: at least one word byte, then `_` and 32 hex digits.
    let mut k = pos + 1;
    while k <= rest.len() && rest[pos..k].iter().all(|&b| is_word(b)) {
        if rest.get(k) == Some(&b'_') && hex_run(&rest[k + 1..], 32) {
            return Some(start + k + 1 + 32);
        }
        k += 1;
    }
    None
}
