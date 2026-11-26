//! Port of Symfony\Component\DependencyInjection\Loader\UndefinedExtensionHandler.php:
//! the message for configuration that no registered extension loads.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dependency-injection Loader/UndefinedExtensionHandler.php

use std::fmt::Write as _;

/// `UndefinedExtensionHandler`.
pub struct UndefinedExtensionHandler;

/// `BUNDLE_EXTENSIONS`.
const BUNDLE_EXTENSIONS: [(&str, &str); 11] = [
    ("debug", "DebugBundle"),
    ("doctrine", "DoctrineBundle"),
    ("doctrine_migrations", "DoctrineMigrationsBundle"),
    ("framework", "FrameworkBundle"),
    ("maker", "MakerBundle"),
    ("monolog", "MonologBundle"),
    ("security", "SecurityBundle"),
    ("twig", "TwigBundle"),
    ("twig_component", "TwigComponentBundle"),
    ("ux_icons", "UXIconsBundle"),
    ("web_profiler", "WebProfilerBundle"),
];

impl UndefinedExtensionHandler {
    /// `getErrorMessage($extensionName, $loadingFilePath,
    /// $namespaceOrAlias, $foundExtensionNamespaces)`.
    pub fn get_error_message(
        extension_name: &str,
        loading_file_path: Option<&str>,
        namespace_or_alias: &str,
        found_extension_namespaces: &[String],
    ) -> String {
        let mut message = String::new();
        if let Some((_, bundle)) = BUNDLE_EXTENSIONS
            .iter()
            .find(|(name, _)| *name == extension_name)
        {
            let _ = write!(
                message,
                "Did you forget to install or enable the {bundle}? "
            );
        }

        let _ = match loading_file_path {
            Some(path) => write!(
                message,
                "There is no extension able to load the configuration for \"{extension_name}\" (in \"{path}\"). "
            ),
            None => write!(
                message,
                "There is no extension able to load the configuration for \"{extension_name}\". "
            ),
        };

        let found = if found_extension_namespaces.is_empty() {
            "none".to_string()
        } else {
            found_extension_namespaces.join("\", \"")
        };
        let _ = write!(
            message,
            "Looked for namespace \"{namespace_or_alias}\", found \"{found}\"."
        );
        message
    }
}
