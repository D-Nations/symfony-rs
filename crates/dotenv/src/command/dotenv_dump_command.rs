//! Port of Symfony\Component\Dotenv\Command\DotenvDumpCommand.php:
//! `dotenv:dump`.
//!
//! `$_SERVER` and `$_ENV` are the application's `Globals`; `loadEnv()`
//! replaces them while it loads and restores them afterwards, as upstream
//! does. The `.env.local.php` written is `var_export()`'s array, which
//! `Dotenv::bootEnv()` reads back without running PHP.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dotenv Command/DotenvDumpCommand.php

use php_value::{Key, PhpArray, Value};
use symfony_console::{
    ConsoleError,
    attribute::as_command::AsCommand,
    command::command::{Command, CommandHandler},
    input::{
        SuggestedValues,
        input::Input,
        input_argument::{InputArgument, OPTIONAL, REQUIRED},
        input_option::{Shortcut, VALUE_NONE},
    },
    output::output::OutputInterface,
    php::sprintf_s,
};

use super::{
    SharedGlobals, basename, console_error, get_dotenv_path, lock, value_string, var_export,
};
use crate::{dotenv::Dotenv, globals::Globals};

type Result<T, E = ConsoleError> = std::result::Result<T, E>;

/// `DotenvDumpCommand`.
pub struct DotenvDumpCommand {
    project_dir: String,
    default_env: Option<String>,
    globals: SharedGlobals,
}

impl DotenvDumpCommand {
    /// `new DotenvDumpCommand($projectDir, $defaultEnv)`.
    pub fn command(
        project_dir: &str,
        default_env: Option<&str>,
        globals: SharedGlobals,
    ) -> Result<Command> {
        Command::with_handler(
            Self {
                project_dir: project_dir.to_owned(),
                default_env: default_env.map(str::to_owned),
                globals,
            },
            None,
        )
    }
}

fn config_string(config: &PhpArray, name: &str, default: &str) -> String {
    match config.get(&Key::from(name)) {
        Some(v) if !v.is_null() => value_string(v),
        _ => default.to_owned(),
    }
}

/// `loadEnv($dotenvPath, $env, $config)`: `$_ENV` after loading, with the
/// globals put back as they were.
fn load_env(
    globals: &mut Globals,
    dotenv_path: &str,
    env: &str,
    config: &PhpArray,
) -> Result<PhpArray> {
    let env_key = config_string(config, "env_var_name", "APP_ENV");
    let test_envs: Vec<String> = match config.get(&Key::from("test_envs")) {
        Some(Value::Array(envs)) => envs.values().map(value_string).collect(),
        Some(v) if !v.is_null() => vec![value_string(v)],
        _ => vec!["test".to_owned()],
    };
    let test_envs: Vec<&str> = test_envs.iter().map(String::as_str).collect();

    let mut dotenv = Dotenv::with_keys(&env_key, "APP_DEBUG");

    let backup = (globals.server.clone(), globals.env.clone());
    globals.server.remove(&Key::from(env_key.as_str()));
    globals.env = PhpArray::new();
    globals.env.insert(env_key.as_str(), Value::string(env));
    let names: Vec<String> = globals
        .server
        .keys()
        .map(|k| match k {
            Key::Int(i) => i.to_string(),
            Key::Str(s) => s.to_string_lossy().into_owned(),
        })
        .collect();
    globals
        .server
        .insert("SYMFONY_DOTENV_VARS", Value::string(names.join(",")));

    let result = dotenv
        .load_env(globals, dotenv_path, None, "dev", &test_envs, false)
        .map(|()| {
            let mut env = globals.env.clone();
            env.remove(&Key::from("SYMFONY_DOTENV_VARS"));
            env.remove(&Key::from("SYMFONY_DOTENV_PATH"));
            env
        });
    (globals.server, globals.env) = backup;
    result.map_err(console_error)
}

impl CommandHandler for DotenvDumpCommand {
    fn class(&self) -> &str {
        "Symfony\\Component\\Dotenv\\Command\\DotenvDumpCommand"
    }

    fn attribute(&self) -> Option<AsCommand> {
        Some(AsCommand::with(
            "dotenv:dump",
            Some("Compile .env files to .env.local.php"),
            &[],
            false,
            None,
            &[],
        ))
    }

    fn configure(&mut self, command: &mut Command) -> Result<()> {
        let mode = if self.default_env.is_none() {
            REQUIRED
        } else {
            OPTIONAL
        };
        command.set_definition_items(vec![
            InputArgument::new(
                "env",
                Some(mode),
                "The application environment to dump .env files for - e.g. \"prod\".",
                Value::Null,
                SuggestedValues::None,
            )?
            .into(),
        ])?;
        command.add_option(
            "empty",
            Shortcut::None,
            Some(VALUE_NONE),
            "Ignore the content of .env files",
            Value::Null,
            SuggestedValues::None,
        )?;
        command.set_help(concat!(
            "The <info>%command.name%</info> command compiles .env files into a PHP-optimized file called .env.local.php.\n",
            "\n",
            "    <info>%command.full_name%</info>",
        ));
        Ok(())
    }

    fn execute(
        &mut self,
        _command: &mut Command,
        input: &mut Input,
        output: &mut dyn OutputInterface,
    ) -> Result<i64> {
        let mut globals = lock(&self.globals);
        let (dotenv_path, config) = get_dotenv_path(&self.project_dir, &globals)?;

        let mut env: Option<String> = match input.get_argument("env")? {
            Value::Null => self.default_env.clone(),
            v => Some(value_string(&v)),
        };
        let env_key = config_string(&config, "env_var_name", "APP_ENV");

        let vars = if matches!(input.get_option("empty")?, Value::Bool(true)) {
            let mut vars = PhpArray::new();
            vars.insert(
                env_key.as_str(),
                env.clone().map_or(Value::Null, Value::string),
            );
            vars
        } else {
            let vars = load_env(
                &mut globals,
                &dotenv_path,
                env.as_deref().unwrap_or_default(),
                &config,
            )?;
            env = vars.get(&Key::from(env_key.as_str())).map(value_string);
            vars
        };
        let env = env.unwrap_or_default();

        let exported = var_export(&Value::Array(vars));
        let contents = format!(
            "<?php\n\n// This file was generated by running \"php bin/console dotenv:dump {env}\"\n\nreturn {exported};\n"
        );
        let file = format!("{dotenv_path}.local.php");
        std::fs::write(&file, contents).map_err(|e| ConsoleError::Other {
            class: "ErrorException".to_owned(),
            message: format!("file_put_contents({file}): Failed to open stream: {e}"),
        })?;

        output.writeln(&sprintf_s(
            "Successfully dumped %s files in <info>%1$s.local.php</> for the <info>%s</> environment.",
            &[basename(&dotenv_path), &env],
        )?)?;

        Ok(0)
    }
}
