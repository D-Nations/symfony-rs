//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveClassPass.php.
//!
//! A definition without a class takes its id as class when the id looks
//! like a class name and the class registry knows it. Chains of parents
//! are checked for cycles first, so the passes that walk parents before
//! `ResolveChildDefinitionsPass` cannot loop.
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveClassPass.php

use std::collections::HashSet;

use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::exception::{Error, Result};

/// `ResolveClassPass`.
#[derive(Debug, Clone, Default)]
pub struct ResolveClassPass;

impl ResolveClassPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveClassPass";

    pub fn new() -> Self {
        Self
    }
}

/// `[a-zA-Z_\x7f-\xff]`.
fn is_label_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x7f
}

/// `[a-zA-Z0-9_\x7f-\xff]`.
fn is_label_char(b: u8) -> bool {
    is_label_start(b) || b.is_ascii_digit()
}

/// `preg_match('/^[a-zA-Z_\x7f-\xff][a-zA-Z0-9_\x7f-\xff]*+(?:\\\\[a-zA-Z_\x7f-\xff][a-zA-Z0-9_\x7f-\xff]*+)++$/', $id)`:
/// a namespaced class name (at least one `\`). `$` also matches before
/// a final newline.
pub(crate) fn looks_like_fqcn(id: &str) -> bool {
    let bytes = id.as_bytes();
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let mut segments = 0;
    for segment in bytes.split(|&b| b == b'\\') {
        match segment.split_first() {
            Some((&first, rest))
                if is_label_start(first) && rest.iter().all(|&b| is_label_char(b)) =>
            {
                segments += 1;
            }
            _ => return false,
        }
    }
    segments >= 2
}

impl CompilerPassInterface for ResolveClassPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let mut acyclic: HashSet<String> = HashSet::new();
        let definitions: Vec<_> = container
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();

        for (id, definition) in definitions {
            let mut path: Vec<String> = Vec::new();
            let mut parent_definition = definition.clone();
            while let Some(parent) = parent_definition.parent() {
                if acyclic.contains(&parent) || !container.has(&parent) {
                    break;
                }
                let i = path.iter().position(|p| *p == parent);
                path.push(parent.clone());
                if let Some(i) = i {
                    return Err(Error::service_circular_reference(
                        parent,
                        path[i..].to_vec(),
                    ));
                }
                parent_definition = container.find_definition(&parent)?;
            }
            acyclic.extend(path);

            if definition.is_synthetic()
                || definition.has_errors()
                || definition.class().is_some()
                || !looks_like_fqcn(&id)
            {
                continue;
            }
            if container.get_reflection_class(Some(&id), false)?.is_some() {
                definition.set_class(Some(&id));
                continue;
            }
            if definition.is_child() {
                return Err(Error::invalid_argument(format!(
                    "Service definition \"{id}\" has a parent but no class, and its name looks like a FQCN. Either the class is missing or you want to inherit it from the parent service. To resolve this ambiguity, please rename this service to a non-FQCN (e.g. using dots), or create the missing class."
                )));
            }

            return Err(Error::invalid_argument(format!(
                "Service id \"{id}\" looks like a FQCN but no corresponding class or interface exists. To resolve this ambiguity, please rename this service to a non-FQCN (e.g. using dots), or create the missing class or interface."
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::looks_like_fqcn;

    #[test]
    fn fqcn_shape() {
        assert!(looks_like_fqcn("App\\Foo"));
        assert!(looks_like_fqcn("App\\Foo\\Bar_1"));
        assert!(!looks_like_fqcn("Foo"));
        assert!(!looks_like_fqcn("App\\Foo.parent"));
        assert!(!looks_like_fqcn("\\App\\Foo"));
        assert!(!looks_like_fqcn("App\\1Foo"));
        assert!(!looks_like_fqcn("App\\"));
    }
}
