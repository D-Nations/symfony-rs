//! Port of Symfony\Component\Dotenv\Command\DebugCommand.php:
//! `debug:dotenv`.
//!
//! `$_SERVER` is the application's `Globals`. A `.env.local.php` is read
//! as dump-env's literal array (`local_php`), not run.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dotenv Command/DebugCommand.php

use std::sync::Arc;

use php_value::{Key, PhpArray, Value};
use symfony_console::{
    ConsoleError,
    attribute::as_command::AsCommand,
    command::command::{Command, CommandHandler},
    completion::{completion_input::CompletionInput, suggestion::CompletionSuggestions},
    formatter::output_formatter::OutputFormatter,
    helper::{formatter_helper::FormatterHelper, table::Row, table_cell::Cell},
    input::{
        SuggestedValues,
        input::Input,
        input_argument::{InputArgument, OPTIONAL},
    },
    output::output::OutputInterface,
    php::compare_strings,
    style::symfony_style::SymfonyStyle,
};

use super::{
    SharedGlobals, basename, console_error, dirname, get_dotenv_path, is_file, lock, value_string,
};
use crate::{dotenv::Dotenv, globals::Globals, local_php};

type Result<T, E = ConsoleError> = std::result::Result<T, E>;

/// `DebugCommand`.
pub struct DebugCommand {
    kernel_environment: String,
    project_dir: String,
    globals: SharedGlobals,
}

impl DebugCommand {
    /// `new DebugCommand($kernelEnvironment, $projectDir)`.
    pub fn command(
        kernel_environment: &str,
        project_dir: &str,
        globals: SharedGlobals,
    ) -> Result<Command> {
        Command::with_handler(
            Self {
                kernel_environment: kernel_environment.to_owned(),
                project_dir: project_dir.to_owned(),
                globals,
            },
            None,
        )
    }

    fn scanner(&self) -> Scanner {
        Scanner {
            kernel_environment: self.kernel_environment.clone(),
            project_dir: self.project_dir.clone(),
        }
    }
}

/// What the command reads from the project: its methods that need no
/// `$this` state beyond the constructor's arguments.
#[derive(Clone)]
struct Scanner {
    kernel_environment: String,
    project_dir: String,
}

impl Scanner {
    /// `getEnvFiles($filePath)`.
    fn get_env_files(&self, file_path: &str) -> Vec<String> {
        let env = &self.kernel_environment;
        let mut files = vec![
            format!("{file_path}.local.php"),
            format!("{file_path}.{env}.local"),
            format!("{file_path}.{env}"),
        ];
        if env != "test" {
            files.push(format!("{file_path}.local"));
        }
        let dist = format!("{file_path}.dist");
        if !is_file(file_path) && is_file(&dist) {
            files.push(dist);
        } else {
            files.push(file_path.to_owned());
        }
        files
    }

    /// `getRelativeName($filePath)`.
    fn get_relative_name<'a>(&self, file_path: &'a str) -> &'a str {
        let project_dir = if is_file(&self.project_dir) {
            dirname(&self.project_dir)
        } else {
            self.project_dir.clone()
        };
        match file_path.strip_prefix(&format!("{project_dir}/")) {
            Some(rest) => rest,
            None => basename(file_path),
        }
    }

    /// `loadValues($filePath)`.
    fn load_values(file_path: &str, globals: &Globals) -> Result<PhpArray> {
        #[expect(
            clippy::case_sensitive_file_extension_comparisons,
            reason = "PHP's str_ends_with() is case-sensitive"
        )]
        let php = file_path.ends_with(".php");
        if php {
            return match local_php::read(file_path).map_err(console_error)? {
                Some(values) => Ok(values),
                None => Err(ConsoleError::Other {
                    class: "TypeError".to_owned(),
                    message: "Symfony\\Component\\Dotenv\\Command\\DebugCommand::loadValues(): Return value must be of type array, int returned".to_owned(),
                }),
            };
        }
        let data = std::fs::read(file_path).map_err(|e| ConsoleError::Other {
            class: "ErrorException".to_owned(),
            message: format!("file_get_contents({file_path}): Failed to open stream: {e}"),
        })?;
        Dotenv::new()
            .parse(globals, &data, ".env")
            .map_err(console_error)
    }

    /// `getVariables($envFiles, $nameFilter)`: each variable's row.
    fn get_variables(
        env_files: &[String],
        name_filter: Option<&str>,
        globals: &Globals,
        formatter: Option<&FormatterHelper>,
    ) -> Result<Vec<(Key, Vec<String>)>> {
        let dotenv_vars: Vec<String> = globals
            .server
            .get(&Key::from("SYMFONY_DOTENV_VARS"))
            .map(value_string)
            .unwrap_or_default()
            .split(',')
            .map(str::to_owned)
            .collect();

        let mut file_values: Vec<PhpArray> = Vec::with_capacity(env_files.len());
        let mut names: Vec<Key> = Vec::new();
        for env_file in env_files {
            let values = Self::load_values(env_file, globals)?;
            // $variables += $fileValues[$envFile]
            for key in values.keys() {
                if !names.contains(key) {
                    names.push(key.clone());
                }
            }
            file_values.push(values);
        }

        let mut variables = Vec::new();
        for var in names {
            let name = key_string(&var);
            if let Some(filter) = name_filter
                && !stripos_is_zero(&name, filter)
            {
                continue;
            }

            let real_value = match globals.server.get(&var) {
                Some(v) if !v.is_null() => value_string(v),
                _ => String::new(),
            };
            let mut details = vec![
                name.clone(),
                format!("<fg=green>{}</>", OutputFormatter::escape(&real_value)),
            ];
            let mut var_seen = !dotenv_vars.contains(&name);

            for values in &file_values {
                let value = match values.get(&var) {
                    Some(v) if !v.is_null() => v,
                    _ => {
                        details.push("<fg=yellow>n/a</>".to_owned());
                        continue;
                    }
                };
                let helper = formatter.ok_or_else(|| {
                    ConsoleError::Logic(
                        "Cannot retrieve helper \"formatter\" because there is no HelperSet defined. Did you forget to add your command to the application or to set the application on the command using the setApplication() method? You can also set the HelperSet directly using the setHelperSet() method.".to_owned()
                    )
                })?;
                let text = value_string(value);
                let shortened = OutputFormatter::escape(&helper.truncate(&text, 30, "..."));
                // $value === $realValue: a non-string value is never identical
                let identical = matches!(value, Value::String(_)) && text == real_value;
                details.push(if identical && !var_seen {
                    format!("<fg=green>{shortened}</>")
                } else {
                    shortened
                });
                var_seen = var_seen || identical;
            }

            variables.push((var, details));
        }

        // ksort($variables)
        variables.sort_by(|(a, _), (b, _)| compare_keys(a, b));
        Ok(variables)
    }

    /// `getAvailableVars()`, with the command's formatter helper.
    fn get_available_vars(
        &self,
        globals: &Globals,
        formatter: Option<&FormatterHelper>,
    ) -> Result<Vec<Value>> {
        let (dotenv_path, _) = get_dotenv_path(&self.project_dir, globals)?;
        let env_files: Vec<String> = self
            .get_env_files(&dotenv_path)
            .into_iter()
            .filter(|f| is_file(f))
            .collect();
        Ok(Self::get_variables(&env_files, None, globals, formatter)?
            .into_iter()
            .map(|(key, _)| key.to_value())
            .collect())
    }
}

fn key_string(key: &Key) -> String {
    match key {
        Key::Int(i) => i.to_string(),
        Key::Str(s) => s.to_string_lossy().into_owned(),
    }
}

/// `0 === stripos($haystack, $needle)`: the needle starts the haystack,
/// ASCII letters compared without case (an empty needle is found at 0).
fn stripos_is_zero(haystack: &str, needle: &str) -> bool {
    haystack.len() >= needle.len()
        && haystack.as_bytes()[..needle.len()].eq_ignore_ascii_case(needle.as_bytes())
}

/// `ksort()`'s default order: integer keys as numbers, string keys as
/// `<=>` compares them.
fn compare_keys(a: &Key, b: &Key) -> std::cmp::Ordering {
    match (a, b) {
        (Key::Int(x), Key::Int(y)) => x.cmp(y),
        _ => compare_strings(key_string(a).as_bytes(), key_string(b).as_bytes()),
    }
}

impl CommandHandler for DebugCommand {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\Dotenv\\Command\\DebugCommand"
    }

    fn attribute(&self) -> Option<AsCommand> {
        Some(AsCommand::with(
            "debug:dotenv",
            Some("List all dotenv files with variables and values"),
            &[],
            false,
            None,
            &[],
        ))
    }

    fn configure(&mut self, command: &mut Command) -> Result<()> {
        let scanner = self.scanner();
        let globals = Arc::clone(&self.globals);
        // $this->getAvailableVars(...): the closure cannot reach the
        // command's helper set, so it truncates with a FormatterHelper of
        // its own (only the values' display uses it, not the names given)
        let available_vars = SuggestedValues::Closure(Arc::new(move |_: &CompletionInput| {
            let globals = lock(&globals);
            Value::Array(PhpArray::list(
                scanner
                    .get_available_vars(&globals, Some(&FormatterHelper))
                    .unwrap_or_default(),
            ))
        }));
        command.set_definition_items(vec![
            InputArgument::new(
                "filter",
                Some(OPTIONAL),
                "The name of an environment variable or a filter.",
                Value::Null,
                available_vars,
            )?
            .into(),
        ])?;
        command.set_help(concat!(
            "The <info>%command.full_name%</info> command displays all the environment variables configured by dotenv:\n",
            "\n",
            "  <info>php %command.full_name%</info>\n",
            "\n",
            "To get specific variables, specify its full or partial name:\n",
            "\n",
            "    <info>php %command.full_name% FOO_BAR</info>\n",
        ));
        Ok(())
    }

    fn execute(
        &mut self,
        command: &mut Command,
        input: &mut Input,
        output: &mut dyn OutputInterface,
    ) -> Result<i64> {
        let name_filter = match input.get_argument("filter")? {
            Value::Null => None,
            v => Some(value_string(&v)),
        };
        let helper = command.get_helper("formatter").ok();
        let formatter = helper
            .as_deref()
            .and_then(|h| h.downcast_ref::<FormatterHelper>());
        let scanner = self.scanner();
        let globals = lock(&self.globals);

        let mut io = SymfonyStyle::new(input, output)?;
        io.title("Dotenv Variables & Files")?;

        if !globals
            .server
            .contains_key(&Key::from("SYMFONY_DOTENV_VARS"))
        {
            io.error(&["Dotenv component is not initialized."])?;
            return Ok(1);
        }

        let (dotenv_path, _) = get_dotenv_path(&scanner.project_dir, &globals)?;

        let env_files = scanner.get_env_files(&dotenv_path);
        let available_files: Vec<String> =
            env_files.iter().filter(|f| is_file(f)).cloned().collect();

        if available_files.contains(&format!("{dotenv_path}.local.php")) {
            io.warning(&[&format!(
                "Due to existing dump file ({}.local.php) all other dotenv files are skipped.",
                scanner.get_relative_name(&dotenv_path)
            )])?;
        }

        if is_file(&dotenv_path) && is_file(&format!("{dotenv_path}.dist")) {
            let name = scanner.get_relative_name(&dotenv_path);
            io.warning(&[&format!(
                "The file {name}.dist gets skipped due to the existence of {name}."
            )])?;
        }

        io.section("Scanned Files (in descending priority)")?;
        let listing: Vec<String> = env_files
            .iter()
            .map(|env_file| {
                if available_files.contains(env_file) {
                    format!("<fg=green>✓</> {}", scanner.get_relative_name(env_file))
                } else {
                    format!("<fg=red>⨯</> {}", scanner.get_relative_name(env_file))
                }
            })
            .collect();
        io.listing(&listing.iter().map(String::as_str).collect::<Vec<_>>())?;

        let variables = Scanner::get_variables(
            &available_files,
            name_filter.as_deref(),
            &globals,
            formatter,
        )?;

        io.section("Variables")?;

        if !variables.is_empty() || name_filter.is_none() {
            let mut headers: Vec<Cell> = vec![Cell::from("Variable"), Cell::from("Value")];
            headers.extend(
                available_files
                    .iter()
                    .map(|f| Cell::from(scanner.get_relative_name(f))),
            );
            let rows = variables
                .into_iter()
                .map(|(_, details)| Row::cells(details))
                .collect();
            io.table(headers, rows)?;

            io.comment(&["Note that values might be different between web and CLI."])?;
        } else {
            io.warning(&[&format!(
                "No variables match the given filter \"{}\".",
                name_filter.unwrap_or_default()
            )])?;
        }

        Ok(0)
    }

    fn complete(
        &mut self,
        command: &mut Command,
        input: &CompletionInput,
        suggestions: &mut CompletionSuggestions,
    ) -> Result<()> {
        if input.must_suggest_argument_values_for("filter") {
            let helper = command.get_helper("formatter").ok();
            let formatter = helper
                .as_deref()
                .and_then(|h| h.downcast_ref::<FormatterHelper>());
            let vars = self
                .scanner()
                .get_available_vars(&lock(&self.globals), formatter)?;
            suggestions.suggest_values(&vars);
        }
        Ok(())
    }
}
