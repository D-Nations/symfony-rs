//! Port of Symfony\Component\DependencyInjection\Loader\YamlFileLoader.php:
//! loads a YAML service configuration (imports, parameters, extension
//! configs, services, `when@env` blocks) through `ContentLoaderTrait`.
//!
//! The file is parsed with symfony-yaml (`PARSE_CONSTANT |
//! PARSE_CUSTOM_TAGS`). Its `!php/const` and `!php/enum` tags read the
//! loader's YAML [`Environment`] ([`YamlFileLoader::set_yaml_environment`];
//! PHP's core constants by default), in place of the PHP process's
//! constants and classes. symfony-yaml is always there, so the "Yaml
//! component is not installed" error cannot happen.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dependency-injection Loader/YamlFileLoader.php

use std::sync::{Arc, PoisonError, RwLock, Weak};

use php_value::{Key, PhpArray, Value};
use symfony_config::file_locator::FileLocatorInterface;
use symfony_config::loader::{
    FileLoader as BaseFileLoader, LoaderInterface, LoaderResolverInterface,
};
use symfony_config::{Error as ConfigError, Located};
use symfony_yaml::{Environment, Flags, Parser};

use super::content_loader_trait::{ContentLoader, Parsed};
use super::ini_file_loader::extension;
use super::{FileLoader, SharedContainer, UndefinedExtensionHandler, cfg, lock};
use crate::container_builder::TrackContents;
use crate::exception::Error;

pub const CLASS: &str = "Symfony\\Component\\DependencyInjection\\Loader\\YamlFileLoader";

/// `YamlFileLoader`.
pub struct YamlFileLoader {
    file: FileLoader,
    content: ContentLoader,
    environment: RwLock<Arc<Environment>>,
}

impl YamlFileLoader {
    /// `new YamlFileLoader($container, $locator, $env, $prepend)`.
    pub fn new(
        container: SharedContainer,
        locator: Arc<dyn FileLocatorInterface>,
        env: Option<&str>,
        prepend: bool,
    ) -> Arc<Self> {
        Arc::new_cyclic(|this: &Weak<Self>| {
            let this: Weak<dyn LoaderInterface> = this.clone();
            let file = FileLoader::new(container, locator, env, prepend, this);
            // protected bool $autoRegisterAliasesForSinglyImplementedInterfaces = false;
            file.set_auto_register_aliases_for_singly_implemented_interfaces(false);
            Self {
                file,
                content: ContentLoader::new(CLASS),
                environment: RwLock::new(Arc::new(Environment::new())),
            }
        })
    }

    /// The constants and enums `!php/const` and `!php/enum` see.
    pub fn set_yaml_environment(&self, environment: Arc<Environment>) {
        *self
            .environment
            .write()
            .unwrap_or_else(PoisonError::into_inner) = environment;
    }

    /// The embedded `FileLoader`.
    pub fn file_loader(&self) -> &FileLoader {
        &self.file
    }

    /// `loadFile($file)`: the file's content, validated; `None` for an
    /// empty file.
    pub fn load_file(&self, file: &str) -> Result<Option<PhpArray>, ConfigError> {
        if !is_local(file) {
            return Err(cfg(Error::invalid_argument(format!(
                "This is not a local file \"{file}\"."
            ))));
        }

        if !php_value::path::to_path(file.as_bytes()).is_some_and(|p| p.is_file()) {
            return Err(cfg(Error::invalid_argument(format!(
                "The file \"{file}\" does not exist."
            ))));
        }

        let environment = self
            .environment
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let mut parser = Parser::with_environment(environment);

        let configuration = parser
            .parse_file(file, Flags::PARSE_CONSTANT | Flags::PARSE_CUSTOM_TAGS)
            .map_err(|e| {
                cfg(Error::invalid_argument(format!(
                    "The file \"{file}\" does not contain valid YAML: {}",
                    e.get_message().to_string_lossy()
                )))
            })?;

        self.validate(configuration, file)
    }

    /// `validate($content, $file)`.
    fn validate(&self, content: Value, file: &str) -> Result<Option<PhpArray>, ConfigError> {
        let content = match content {
            Value::Null => return Ok(None),
            Value::Array(a) => a,
            _ => {
                return Err(cfg(Error::invalid_argument(format!(
                    "The service file \"{file}\" is not valid. It should contain an array."
                ))));
            }
        };

        for namespace in content.keys() {
            let namespace =
                String::from_utf8_lossy(&symfony_config::php::key_bytes(namespace)).into_owned();
            if ["imports", "parameters", "services"].contains(&namespace.as_str())
                || namespace.starts_with("when@")
            {
                continue;
            }

            let container = lock(self.file.container());
            if !self.file.prepend() && !container.has_extension(&namespace) {
                let extension_namespaces: Vec<String> = container
                    .get_extensions()
                    .values()
                    .map(|e| e.get_alias())
                    .filter(|a| !a.is_empty() && a != "0")
                    .collect();
                return Err(cfg(Error::invalid_argument(
                    UndefinedExtensionHandler::get_error_message(
                        &namespace,
                        Some(file),
                        &namespace,
                        &extension_namespaces,
                    ),
                )));
            }
        }

        Ok(Some(content))
    }

    /// `parseDefinition($id, $service, $file, $defaults)` (private
    /// upstream; its tests call it through reflection).
    pub fn parse_definition(
        &self,
        id: &str,
        service: &Value,
        file: &str,
        defaults: &PhpArray,
    ) -> Result<Option<Value>, ConfigError> {
        Ok(self
            .content
            .parse_definition(&self.file, id, service, file, defaults, false, true)?
            .map(|p| match p {
                Parsed::Definition(d) => d.to_value(),
                Parsed::Alias(a) => a.to_value(),
            }))
    }
}

/// `stream_is_local($file)`: no scheme, or `file://`.
fn is_local(file: &str) -> bool {
    match file.find("://") {
        Some(i) => {
            let scheme = &file[..i];
            !scheme
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
                || scheme.eq_ignore_ascii_case("file")
        }
        None => true,
    }
}

impl LoaderInterface for YamlFileLoader {
    fn class(&self) -> &str {
        CLASS
    }

    fn load(&self, resource: &Value, _type: Option<&[u8]>) -> Result<Value, ConfigError> {
        let name = symfony_config::php::string_value(resource)?;
        let path = match self.file.base().get_locator().locate(&name, None, true)? {
            Located::Path(p) => p,
            Located::Paths(mut ps) => ps.remove(0),
        };
        let path = String::from_utf8_lossy(&path).into_owned();

        let content = self.load_file(&path)?;

        lock(self.file.container())
            .file_exists(&path, &TrackContents::Yes)
            .map_err(cfg)?;

        // empty file
        let Some(content) = content else {
            return Ok(Value::Null);
        };

        self.file.state().importing += 1;
        let result = (|| {
            self.content.load_content(&self.file, &content, &path)?;

            // per-env configuration
            if let Some(env) = self.file.env().filter(|e| !e.is_empty() && e != "0") {
                let when = format!("when@{env}");
                if let Some(value) = content
                    .get(&Key::from(when.as_str()))
                    .filter(|v| !v.is_null())
                {
                    let Value::Array(value) = value else {
                        return Err(cfg(Error::invalid_argument(format!(
                            "The \"{when}\" key should contain an array in \"{path}\"."
                        ))));
                    };

                    self.content.load_content(&self.file, value, &path)?;
                }
            }
            Ok(())
        })();
        // finally
        self.file.state().importing -= 1;
        result?;
        self.file.load_extension_configs()?;

        Ok(Value::Null)
    }

    fn supports(&self, resource: &Value, r#type: Option<&[u8]>) -> bool {
        let Value::String(resource) = resource else {
            return false;
        };

        if r#type.is_none() && matches!(extension(resource.as_bytes()), b"yaml" | b"yml") {
            return true;
        }

        matches!(r#type, Some(b"yaml" | b"yml"))
    }

    fn get_resolver(&self) -> Result<Arc<dyn LoaderResolverInterface>, ConfigError> {
        self.file.base().loader().get_resolver()
    }

    fn set_resolver(&self, resolver: Arc<dyn LoaderResolverInterface>) {
        self.file.base().loader().set_resolver(resolver);
    }

    fn as_file_loader(&self) -> Option<&BaseFileLoader> {
        Some(self.file.base())
    }
}
