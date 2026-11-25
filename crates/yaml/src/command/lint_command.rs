//! Port of Symfony\Component\Yaml\Command\LintCommand.php: `lint:yaml`.
//!
//! Behind the `console` feature. The parser is built on an `Environment`:
//! the constants `!php/const` may name, which PHP finds among the loaded
//! classes and a converter states at build time.
//!
//! Upstream turns a deprecation raised while parsing into a `ParseException`
//! with an error handler. The Yaml port raises none (symfony/yaml 8 itself
//! triggers no deprecation), so there is no handler here.
//!
//! A directory is walked as PHP's `RecursiveDirectoryIterator` with
//! `SKIP_DOTS | FOLLOW_SYMLINKS` walks it, in the order the system lists
//! entries; `is_readable()` is whether the file or directory can be
//! opened for reading.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/yaml Command/LintCommand.php

use std::{io::Read, sync::Arc};

use php_value::{
    Value,
    json::{JsonFlags, encode_string},
};
use symfony_console::{
    ConsoleError,
    attribute::as_command::AsCommand,
    ci::github_action_reporter::GithubActionReporter,
    command::command::{Command, CommandHandler},
    completion::{completion_input::CompletionInput, suggestion::CompletionSuggestions},
    input::{
        SuggestedValues,
        input::Input,
        input_argument::IS_ARRAY,
        input_option::{Shortcut, VALUE_IS_ARRAY, VALUE_NEGATABLE, VALUE_REQUIRED},
    },
    output::output::OutputInterface,
    style::symfony_style::SymfonyStyle,
};

use crate::{Environment, Flags, Parser};

type Result<T, E = ConsoleError> = std::result::Result<T, E>;

/// The default directory walk, passed to a directory provider.
pub type DirectoryWalk = dyn Fn(&str) -> Result<Vec<String>>;
/// `$directoryIteratorProvider`: the files of a directory, given the
/// default walk.
pub type DirectoryIteratorProvider =
    Box<dyn Fn(&str, &DirectoryWalk) -> Result<Vec<String>> + Send>;
/// `$isReadableProvider`: whether a path is readable, given the default.
pub type IsReadableProvider = Box<dyn Fn(&str, &dyn Fn(&str) -> bool) -> bool + Send>;

const FORMATS: [&str; 3] = ["txt", "json", "github"];

/// One file's result: `validate()`'s array.
#[derive(Debug, Clone, PartialEq)]
struct FileInfo {
    file: Option<String>,
    /// `line` and `message` when the file is not valid.
    error: Option<(i64, String)>,
}

/// `LintCommand`.
pub struct LintCommand {
    environment: Arc<Environment>,
    parser: Option<Parser>,
    format: Option<String>,
    display_correct_files: bool,
    directory_iterator_provider: Option<DirectoryIteratorProvider>,
    is_readable_provider: Option<IsReadableProvider>,
}

impl LintCommand {
    /// `new LintCommand($name, $directoryIteratorProvider, $isReadableProvider)`,
    /// parsing with `environment`'s constants.
    pub fn command(
        name: Option<&str>,
        environment: Arc<Environment>,
        directory_iterator_provider: Option<DirectoryIteratorProvider>,
        is_readable_provider: Option<IsReadableProvider>,
    ) -> Result<Command> {
        Command::with_handler(
            Self {
                environment,
                parser: None,
                format: None,
                display_correct_files: false,
                directory_iterator_provider,
                is_readable_provider,
            },
            name,
        )
    }

    /// `validate($content, $flags, $file)`.
    fn validate(&mut self, content: &[u8], flags: Flags, file: Option<&str>) -> FileInfo {
        let environment = Arc::clone(&self.environment);
        let parser = self
            .parser
            .get_or_insert_with(|| Parser::with_environment(environment));
        match parser.parse(content, Flags(Flags::PARSE_CONSTANT.0 | flags.0)) {
            Ok(_) => FileInfo {
                file: file.map(str::to_owned),
                error: None,
            },
            Err(e) => FileInfo {
                file: file.map(str::to_owned),
                error: Some((
                    e.get_parsed_line(),
                    e.get_message().to_string_lossy().into_owned(),
                )),
            },
        }
    }

    /// `display($io, $files)`.
    fn display(&self, io: &mut SymfonyStyle<'_>, files: &[FileInfo]) -> Result<i64> {
        match self.format.as_deref() {
            Some("txt") => self.display_txt(io, files, false),
            Some("json") => display_json(io, files),
            Some("github") => self.display_txt(io, files, true),
            _ => Err(ConsoleError::InvalidArgument(format!(
                "Supported formats are \"{}\".",
                FORMATS.join("\", \"")
            ))),
        }
    }

    /// `displayTxt($io, $filesInfo, $errorAsGithubAnnotations)`.
    fn display_txt(
        &self,
        io: &mut SymfonyStyle<'_>,
        files_info: &[FileInfo],
        error_as_github_annotations: bool,
    ) -> Result<i64> {
        let count_files = files_info.len();
        let mut errored_files = 0;
        let mut suggest_tag_option = false;

        for info in files_info {
            let in_file = match info.file.as_deref() {
                Some(file) if !file.is_empty() && file != "0" => format!(" in {file}"),
                _ => String::new(),
            };
            match &info.error {
                None if self.display_correct_files => {
                    io.comment(&[&format!("<info>OK</info>{in_file}")])?;
                }
                None => {}
                Some((line, message)) => {
                    errored_files += 1;
                    io.text(&[&format!("<error> ERROR </error>{in_file}")])?;
                    io.text(&[&format!("<error> >> {message}</error>")])?;

                    if message.contains("PARSE_CUSTOM_TAGS") {
                        suggest_tag_option = true;
                    }

                    if error_as_github_annotations {
                        let file = info.file.as_deref().unwrap_or("php://stdin");
                        GithubActionReporter::new(io).error(
                            message,
                            Some(file),
                            Some(*line),
                            None,
                        )?;
                    }
                }
            }
        }

        if errored_files == 0 {
            io.success(&[&format!(
                "All {count_files} YAML files contain valid syntax."
            )])?;
        } else {
            io.warning(&[&format!(
                "{} YAML files have valid syntax and {errored_files} contain errors.{}",
                count_files - errored_files,
                if suggest_tag_option {
                    " Use the --parse-tags option if you want parse custom tags."
                } else {
                    ""
                }
            )])?;
        }

        Ok(i64::from(errored_files > 0))
    }

    /// `getFiles($fileOrDirectory)`.
    fn get_files(&self, file_or_directory: &str) -> Result<Vec<String>> {
        if std::path::Path::new(file_or_directory).is_file() {
            return Ok(vec![file_or_directory.to_owned()]);
        }
        let files = match &self.directory_iterator_provider {
            Some(provider) => provider(file_or_directory, &walk)?,
            None => walk(file_or_directory)?,
        };
        Ok(files
            .into_iter()
            .filter(|file| matches!(extension(file), "yml" | "yaml"))
            .collect())
    }

    /// `isReadable($fileOrDirectory)`.
    fn is_readable(&self, file_or_directory: &str) -> bool {
        match &self.is_readable_provider {
            Some(provider) => provider(file_or_directory, &is_readable),
            None => is_readable(file_or_directory),
        }
    }
}

impl CommandHandler for LintCommand {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\Yaml\\Command\\LintCommand"
    }

    fn attribute(&self) -> Option<AsCommand> {
        Some(AsCommand::with(
            "lint:yaml",
            Some("Lint a YAML file and outputs encountered errors"),
            &[],
            false,
            None,
            &[],
        ))
    }

    fn configure(&mut self, command: &mut Command) -> Result<()> {
        command.add_argument(
            "filename",
            Some(IS_ARRAY),
            "A file, a directory or \"-\" for reading from STDIN",
            Value::Null,
            SuggestedValues::None,
        )?;
        command.add_option(
            "format",
            Shortcut::None,
            Some(VALUE_REQUIRED),
            &format!("The output format (\"{}\")", FORMATS.join("\", \"")),
            Value::Null,
            SuggestedValues::None,
        )?;
        command.add_option(
            "exclude",
            Shortcut::None,
            Some(VALUE_REQUIRED | VALUE_IS_ARRAY),
            "Path(s) to exclude",
            Value::Null,
            SuggestedValues::None,
        )?;
        command.add_option(
            "parse-tags",
            Shortcut::None,
            Some(VALUE_NEGATABLE),
            "Parse custom tags",
            Value::Null,
            SuggestedValues::None,
        )?;
        command.set_help(concat!(
            "The <info>%command.name%</info> command lints a YAML file and outputs to STDOUT\n",
            "the first encountered syntax error.\n",
            "\n",
            "You can validates YAML contents passed from STDIN:\n",
            "\n",
            "  <info>cat filename | php %command.full_name% -</info>\n",
            "\n",
            "You can also validate the syntax of a file:\n",
            "\n",
            "  <info>php %command.full_name% filename</info>\n",
            "\n",
            "Or of a whole directory:\n",
            "\n",
            "  <info>php %command.full_name% dirname</info>\n",
            "\n",
            "The <info>--format</info> option specifies the format of the command output:\n",
            "\n",
            "  <info>php %command.full_name% dirname --format=json</info>\n",
            "\n",
            "You can also exclude one or more specific files:\n",
            "\n",
            "  <info>php %command.full_name% dirname --exclude=\"dirname/foo.yaml\" --exclude=\"dirname/bar.yaml\"</info>\n",
        ));
        Ok(())
    }

    fn execute(
        &mut self,
        _command: &mut Command,
        input: &mut Input,
        output: &mut dyn OutputInterface,
    ) -> Result<i64> {
        // (array) $input->getArgument('filename')
        let filenames: Vec<String> = match input.get_argument("filename")? {
            Value::Null => Vec::new(),
            Value::Array(array) => array.values().map(value_string).collect(),
            other => vec![value_string(&other)],
        };
        let excludes: Vec<String> = match input.get_option("exclude")? {
            Value::Array(array) => array.values().map(value_string).collect(),
            Value::Null => Vec::new(),
            other => vec![value_string(&other)],
        };
        self.format = match input.get_option("format")? {
            Value::Null => None,
            other => Some(value_string(&other)),
        };
        let parse_tags = matches!(input.get_option("parse-tags")?, Value::Bool(true));

        if self.format.is_none() {
            // Autodetect format according to CI environment
            self.format = Some(
                if GithubActionReporter::is_github_action_environment() {
                    "github"
                } else {
                    "txt"
                }
                .to_owned(),
            );
        }

        let flags = if parse_tags {
            Flags::PARSE_CUSTOM_TAGS
        } else {
            Flags::NONE
        };

        self.display_correct_files = output.is_verbose();

        if filenames == ["-"] {
            let mut content = Vec::new();
            std::io::stdin()
                .read_to_end(&mut content)
                .map_err(|e| ConsoleError::Runtime(format!("php://stdin: {e}")))?;
            let info = self.validate(&content, flags, None);
            let mut io = SymfonyStyle::new(input, output)?;
            return self.display(&mut io, &[info]);
        }

        if filenames.is_empty() {
            return Err(ConsoleError::Runtime(
                "Please provide a filename or pipe file content to STDIN.".to_owned(),
            ));
        }

        let mut files_info = Vec::new();
        for filename in &filenames {
            if !self.is_readable(filename) {
                return Err(ConsoleError::Runtime(format!(
                    "File or directory \"{filename}\" is not readable."
                )));
            }

            for file in self.get_files(filename)? {
                if !excludes.contains(&file) {
                    let content = std::fs::read(&file).map_err(|e| ConsoleError::Other {
                        class: "ErrorException".to_owned(),
                        message: format!("file_get_contents({file}): Failed to open stream: {e}"),
                    })?;
                    files_info.push(self.validate(&content, flags, Some(&file)));
                }
            }
        }

        let mut io = SymfonyStyle::new(input, output)?;
        self.display(&mut io, &files_info)
    }

    fn complete(
        &mut self,
        _command: &mut Command,
        input: &CompletionInput,
        suggestions: &mut CompletionSuggestions,
    ) -> Result<()> {
        if input.must_suggest_option_values_for("format") {
            suggestions.suggest_values(&FORMATS.map(Value::string));
        }
        Ok(())
    }
}

/// A value as a string (`(string)` of a scalar).
fn value_string(value: &Value) -> String {
    value
        .to_php_string()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `displayJson($io, $filesInfo)`: `json_encode($filesInfo,
/// JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES)`, which is `false` (an
/// empty line) when a string is not UTF-8.
fn display_json(io: &mut SymfonyStyle<'_>, files_info: &[FileInfo]) -> Result<i64> {
    let flags = JsonFlags {
        unescaped_slashes: true,
        unescaped_unicode: false,
    };
    let mut errors = 0;
    let mut objects = Vec::with_capacity(files_info.len());
    let mut encodable = true;
    for info in files_info {
        let mut string = |s: &str| {
            encode_string(s.as_bytes(), flags).unwrap_or_else(|| {
                encodable = false;
                String::new()
            })
        };
        let file = string(info.file.as_deref().unwrap_or_default());
        let object = match &info.error {
            None => format!("    {{\n        \"file\": {file},\n        \"valid\": true\n    }}"),
            Some((line, message)) => {
                errors += 1;
                let mut message = message.clone();
                if message.contains("PARSE_CUSTOM_TAGS") {
                    message.push_str(" Use the --parse-tags option if you want parse custom tags.");
                }
                let message = string(&message);
                format!(
                    "    {{\n        \"file\": {file},\n        \"line\": {line},\n        \"valid\": false,\n        \"message\": {message}\n    }}"
                )
            }
        };
        objects.push(object);
    }
    let json = if !encodable {
        String::new()
    } else if objects.is_empty() {
        "[]".to_owned()
    } else {
        format!("[\n{}\n]", objects.join(",\n"))
    };
    io.writeln(&json)?;
    Ok(errors.min(1))
}

/// `SplFileInfo::getExtension()`.
fn extension(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rfind('.') {
        Some(i) => &name[i + 1..],
        None => "",
    }
}

/// `is_readable($path)`.
fn is_readable(path: &str) -> bool {
    let path = std::path::Path::new(path);
    if path.is_dir() {
        std::fs::read_dir(path).is_ok()
    } else {
        std::fs::File::open(path).is_ok()
    }
}

/// `new RecursiveIteratorIterator(new RecursiveDirectoryIterator($directory,
/// SKIP_DOTS | FOLLOW_SYMLINKS), LEAVES_ONLY)`: the files under a
/// directory, depth first, in the order the system lists them. A trailing
/// slash of the directory is dropped, as PHP drops it.
pub fn walk(directory: &str) -> Result<Vec<String>> {
    let trimmed = directory.trim_end_matches('/');
    let base = if trimmed.is_empty() && !directory.is_empty() {
        "/"
    } else {
        trimmed
    };
    let mut files = Vec::new();
    walk_into(base, &mut files)?;
    Ok(files)
}

fn walk_into(directory: &str, files: &mut Vec<String>) -> Result<()> {
    let entries = std::fs::read_dir(directory).map_err(|e| ConsoleError::Other {
        class: "UnexpectedValueException".to_owned(),
        message: format!(
            "RecursiveDirectoryIterator::__construct({directory}): Failed to open directory: {e}"
        ),
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| ConsoleError::Other {
            class: "UnexpectedValueException".to_owned(),
            message: format!("{directory}: {e}"),
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if directory == "/" {
            format!("/{name}")
        } else {
            format!("{directory}/{name}")
        };
        // FOLLOW_SYMLINKS: a link to a directory is descended into
        if std::path::Path::new(&path).is_dir() {
            walk_into(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}
