//! Port of Symfony\Component\Routing\RouteCompiler.php and
//! RouteCompilerInterface.php.
//!
//! The compiler writes regular expressions as strings; it runs only
//! upstream's own helper patterns (through php-pcre), never the route's.
//! A subclass that changes `SEPARATORS` (upstream's Utf8RouteCompiler
//! fixture) is [`RouteCompiler::compile_with_separators`] under its own
//! [`RouteCompilerInterface::CLASS`].
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing RouteCompiler.php RouteCompilerInterface.php

use std::sync::LazyLock;

use php_pcre::{Pcre, PcreError, preg_quote};
use php_value::{Key, PhpArray, PhpString, Value};

use crate::{
    compiled_route::CompiledRoute,
    exception::{Class, Error, concat},
    php,
    route::{Route, match_error, pcre},
};

/// `RouteCompilerInterface`: compiles a Route to a CompiledRoute.
pub trait RouteCompilerInterface {
    /// The class name a route's `compiler_class` option names.
    const CLASS: &'static str;

    /// `compile($route)`.
    fn compile(route: &mut Route) -> Result<CompiledRoute, Error>;
}

/// `RouteCompiler`.
pub struct RouteCompiler;

impl RouteCompilerInterface for RouteCompiler {
    const CLASS: &'static str = "Symfony\\Component\\Routing\\RouteCompiler";

    fn compile(route: &mut Route) -> Result<CompiledRoute, Error> {
        Self::compile_with_separators(route, Self::SEPARATORS)
    }
}

type Lazy = LazyLock<Result<Pcre, PcreError>>;

static VARIABLE: Lazy = LazyLock::new(|| Pcre::new(br"#\{(!)?([\w\x80-\xFF]+)\}#"));
static LEADING_VARIABLE: Lazy = LazyLock::new(|| Pcre::new(br"#^\{[\w\x80-\xFF]+\}#"));
static PLACEHOLDER: Lazy = LazyLock::new(|| Pcre::new(br"#\{[\w\x80-\xFF]+\}#"));
static UTF8_REQUIREMENT: Lazy = LazyLock::new(|| {
    Pcre::new(br"/[\x80-\xFF]|(?<!\\)\\(?:\\\\)*+(?-i:X|[pP][\{CLMNPSZ]|x\{[A-Fa-f0-9]{3})/")
});

/// A token as RouteCompiler builds it, before it becomes a PHP array.
enum Token {
    Text(Vec<u8>),
    Variable {
        /// `null` in one case upstream does not foresee (see
        /// [`preceding_char`]).
        separator: Option<Vec<u8>>,
        regex: Vec<u8>,
        name: Vec<u8>,
        utf8: Option<bool>,
        important: bool,
    },
}

impl Token {
    fn to_value(&self) -> Value {
        let s = |b: &[u8]| Value::String(PhpString::from(b));
        let items = match self {
            Self::Text(text) => vec![s(b"text"), s(text)],
            Self::Variable {
                separator,
                regex,
                name,
                utf8,
                important,
            } => {
                let mut items = vec![
                    s(b"variable"),
                    separator.as_deref().map_or(Value::Null, s),
                    s(regex),
                    s(name),
                ];
                if *important {
                    items.push(Value::Bool(utf8.unwrap_or(false)));
                    items.push(Value::Bool(true));
                } else if let Some(utf8) = utf8 {
                    items.push(Value::Bool(*utf8));
                }
                items
            }
        };
        Value::Array(PhpArray::list(items))
    }
}

struct Compiled {
    static_prefix: Vec<u8>,
    regex: Vec<u8>,
    tokens: PhpArray,
    variables: Vec<Vec<u8>>,
}

fn is_utf8(s: &[u8]) -> bool {
    std::str::from_utf8(s).is_ok()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

/// The first UTF-8 character of a valid UTF-8 string.
fn first_char(s: &[u8]) -> &[u8] {
    let len = match s.first() {
        None => 0,
        Some(&b) if b < 0x80 => 1,
        Some(&b) if b >= 0xF0 => 4,
        Some(&b) if b >= 0xE0 => 3,
        Some(_) => 2,
    };
    &s[..len.min(s.len())]
}

/// The start of the last UTF-8 character of a valid UTF-8 string.
fn last_char_start(s: &[u8]) -> usize {
    let mut i = s.len().saturating_sub(1);
    while i > 0 && (s[i] & 0xC0) == 0x80 {
        i -= 1;
    }
    i
}

/// `preg_match('/.$/u', $text, $m); $m[0]`: `.` is not a newline and `$`
/// also matches before a final newline. `None` is PHP's null (no match).
fn preceding_char(text: &[u8]) -> Option<Vec<u8>> {
    let body = text.strip_suffix(b"\n").unwrap_or(text);
    if body.len() == text.len() {
        // ends with a character other than a newline
        let start = last_char_start(text);
        return Some(text[start..].to_vec());
    }
    if body.is_empty() || body.ends_with(b"\n") {
        return None;
    }
    let start = last_char_start(body);
    Some(body[start..].to_vec())
}

impl RouteCompiler {
    /// `SEPARATORS`: the characters that are automatically considered
    /// separators in front of optional placeholders (with default and no
    /// static text following). Such a single separator can be left out
    /// together with the optional placeholder from matching and
    /// generating URLs.
    pub const SEPARATORS: &'static [u8] = b"/,;.:-_~+*=@|";

    /// `VARIABLE_MAXIMUM_LENGTH`: the maximum supported length of a PCRE
    /// subpattern name.
    pub const VARIABLE_MAXIMUM_LENGTH: usize = 32;

    /// `compile()` as a subclass with its own `SEPARATORS` runs it.
    pub fn compile_with_separators(
        route: &mut Route,
        separators: &[u8],
    ) -> Result<CompiledRoute, Error> {
        let mut host_variables = Vec::new();
        let mut variables = Vec::new();
        let mut host_regex = None;
        let mut host_tokens = PhpArray::new();

        let host = route.host().clone();
        if !host.is_empty() {
            let result = compile_pattern(route, &host, true, separators)?;
            host_variables = result.variables;
            variables.clone_from(&host_variables);
            host_tokens = result.tokens;
            host_regex = Some(PhpString::from(result.regex));
        }

        if let (Some(locale), Some(_)) = (
            route.default_value(b"_locale").cloned(),
            route.default_value(b"_canonical_route"),
        ) {
            let locale = php::string_argument(&locale, "preg_quote", 1, "str")?;
            if route
                .requirement(b"_locale")
                .is_some_and(|r| r.as_bytes() == preg_quote(&locale, None).as_slice())
            {
                let mut requirements = route.requirements().clone();
                requirements.remove(&Key::from("_locale"));
                route.set_requirements(&requirements)?;
                let path = replace(route.path(), b"{_locale}", &locale);
                route.set_path(path)?;
            }
        }

        let path = route.path().clone();
        let result = compile_pattern(route, &path, false, separators)?;
        let path_variables = result.variables;
        if path_variables.iter().any(|v| v == b"_fragment") {
            return Err(Error::new(
                Class::PhpInvalidArgument,
                concat(&[
                    b"Route pattern \"",
                    route.path(),
                    b"\" cannot contain \"_fragment\" as a path parameter.",
                ]),
            ));
        }

        variables.extend(path_variables.iter().cloned());
        // array_unique() keeps the first key of each value
        let mut unique = PhpArray::new();
        let mut seen: Vec<&[u8]> = Vec::new();
        for (i, v) in variables.iter().enumerate() {
            if !seen.contains(&v.as_slice()) {
                seen.push(v);
                unique.insert(i as i64, Value::String(PhpString::from(v.as_slice())));
            }
        }

        let list = |items: &[Vec<u8>]| {
            PhpArray::list(
                items
                    .iter()
                    .map(|v| Value::String(PhpString::from(v.as_slice()))),
            )
        };
        Ok(CompiledRoute::new(
            result.static_prefix,
            result.regex,
            result.tokens,
            list(&path_variables),
            host_regex,
            host_tokens,
            list(&host_variables),
            unique,
        ))
    }
}

fn compile_pattern(
    route: &Route,
    pattern: &[u8],
    is_host: bool,
    separators: &[u8],
) -> Result<Compiled, Error> {
    let mut tokens = Vec::new();
    let mut variables: Vec<Vec<u8>> = Vec::new();
    let mut pos = 0;
    let default_separator: &[u8] = if is_host { b"." } else { b"/" };
    let mut use_utf8 = is_utf8(pattern);
    let needs_utf8 = route.option(b"utf8").is_some_and(php::truthy);

    if !needs_utf8 && use_utf8 && pattern.iter().any(|&b| b >= 0x80) {
        return Err(Error::new(
            Class::PhpLogic,
            concat(&[
                b"Cannot use UTF-8 route patterns without setting the \"utf8\" option for route \"",
                route.path(),
                b"\".",
            ]),
        ));
    }
    if !use_utf8 && needs_utf8 {
        return Err(Error::new(
            Class::PhpLogic,
            concat(&[
                b"Cannot mix UTF-8 requirements with non-UTF-8 pattern \"",
                pattern,
                b"\".",
            ]),
        ));
    }

    // Match all variables enclosed in "{}" and iterate over them. But we only want to match the innermost variable
    // in case of nested "{}", e.g. {foo{bar}}. This in ensured because \w does not match "{" or "}" itself.
    let mut matches = Vec::new();
    let mut from = 0;
    while let Some(groups) = pcre(&VARIABLE)?
        .captures_from(pattern, from)
        .map_err(match_error)?
    {
        let Some((start, end)) = groups[0] else { break };
        let important = groups.get(1).copied().flatten().is_some();
        let Some((name_start, name_end)) = groups.get(2).copied().flatten() else {
            break;
        };
        matches.push((
            start,
            end,
            important,
            pattern[name_start..name_end].to_vec(),
        ));
        from = end.max(start + 1);
    }

    for (start, end, important, var_name) in matches {
        // get all static text preceding the current variable
        let preceding_text = &pattern[pos..start];
        pos = end;

        let preceding_char = if preceding_text.is_empty() {
            Some(Vec::new())
        } else if use_utf8 {
            preceding_char(preceding_text)
        } else {
            Some(preceding_text[preceding_text.len() - 1..].to_vec())
        };
        // `'' !== $precedingChar && str_contains(static::SEPARATORS, $precedingChar)`;
        // null reads as '', which every string contains
        let is_separator = match &preceding_char {
            None => true,
            Some(c) => !c.is_empty() && contains(separators, c),
        };

        // A PCRE subpattern name must start with a non-digit. Also a PHP variable cannot start with a digit so the
        // variable would not be usable as a Controller action argument.
        if var_name.first().is_some_and(u8::is_ascii_digit) {
            return Err(Error::new(
                Class::PhpDomain,
                concat(&[
                    b"Variable name \"",
                    &var_name,
                    b"\" cannot start with a digit in route pattern \"",
                    pattern,
                    b"\". Please use a different name.",
                ]),
            ));
        }
        if variables.contains(&var_name) {
            return Err(Error::new(
                Class::PhpLogic,
                concat(&[
                    b"Route pattern \"",
                    pattern,
                    b"\" cannot reference variable name \"",
                    &var_name,
                    b"\" more than once.",
                ]),
            ));
        }
        if var_name.len() > RouteCompiler::VARIABLE_MAXIMUM_LENGTH {
            return Err(Error::new(
                Class::PhpDomain,
                concat(&[
                    b"Variable name \"",
                    &var_name,
                    b"\" cannot be longer than ",
                    RouteCompiler::VARIABLE_MAXIMUM_LENGTH
                        .to_string()
                        .as_bytes(),
                    b" characters in route pattern \"",
                    pattern,
                    b"\". Please use a shorter name.",
                ]),
            ));
        }

        let char_len = preceding_char.as_ref().map_or(0, Vec::len);
        if is_separator && preceding_char.as_deref() != Some(preceding_text) {
            // substr($precedingText, 0, -strlen($precedingChar)); -0 is 0
            let keep = if char_len == 0 {
                0
            } else {
                preceding_text.len() - char_len
            };
            tokens.push(Token::Text(preceding_text[..keep].to_vec()));
        } else if !is_separator && !preceding_text.is_empty() {
            tokens.push(Token::Text(preceding_text.to_vec()));
        }

        let regexp = match route.requirement(&var_name) {
            None => {
                let following_pattern = &pattern[pos..];
                // Find the next static character after the variable that functions as a separator. By default, this separator and '/'
                // are disallowed for the variable. This default requirement makes sure that optional variables can be matched at all
                // and that the generating-matching-combination of URLs unambiguous, i.e. the params used for generating the URL are
                // the same that will be matched. Example: new Route('/{page}.{_format}', ['_format' => 'html'])
                // If {page} would also match the separating dot, {_format} would never match as {page} will eagerly consume everything.
                // Also even if {_format} was not optional the requirement prevents that {page} matches something that was originally
                // part of {_format} when generating the URL, e.g. _format = 'mobile.html'.
                let next_separator = find_next_separator(following_pattern, use_utf8, separators)?;
                let mut regexp = concat(&[
                    b"[^",
                    &preg_quote(default_separator, None),
                    &if default_separator != next_separator.as_slice() && !next_separator.is_empty()
                    {
                        preg_quote(&next_separator, None)
                    } else {
                        Vec::new()
                    },
                    b"]+",
                ]);
                if (!next_separator.is_empty()
                    && !pcre(&LEADING_VARIABLE)?
                        .is_match(following_pattern)
                        .map_err(match_error)?)
                    || following_pattern.is_empty()
                {
                    // When we have a separator, which is disallowed for the variable, we can optimize the regex with a possessive
                    // quantifier. This prevents useless backtracking of PCRE and improves performance by 20% for matching those patterns.
                    // Given the above example, there is no point in backtracking into {page} (that forbids the dot) when a dot must follow
                    // after it. This optimization cannot be applied when the next char is no real separator or when the next variable is
                    // directly adjacent, e.g. '/{x}{y}'.
                    regexp.push(b'+');
                }
                regexp
            }
            Some(requirement) => {
                let requirement = requirement.as_bytes();
                if !is_utf8(requirement) {
                    use_utf8 = false;
                } else if !needs_utf8
                    && pcre(&UTF8_REQUIREMENT)?
                        .is_match(requirement)
                        .map_err(match_error)?
                {
                    return Err(Error::new(
                        Class::PhpLogic,
                        concat(&[
                            b"Cannot use UTF-8 route requirements without setting the \"utf8\" option for variable \"",
                            &var_name,
                            b"\" in pattern \"",
                            pattern,
                            b"\".",
                        ]),
                    ));
                }
                if !use_utf8 && needs_utf8 {
                    return Err(Error::new(
                        Class::PhpLogic,
                        concat(&[
                            b"Cannot mix UTF-8 requirement with non-UTF-8 charset for variable \"",
                            &var_name,
                            b"\" in pattern \"",
                            pattern,
                            b"\".",
                        ]),
                    ));
                }
                transform_capturing_groups_to_non_capturings(requirement)
            }
        };

        tokens.push(Token::Variable {
            separator: if is_separator {
                preceding_char
            } else {
                Some(Vec::new())
            },
            regex: regexp,
            name: var_name.clone(),
            utf8: None,
            important,
        });
        variables.push(var_name);
    }

    if pos < pattern.len() {
        tokens.push(Token::Text(pattern[pos..].to_vec()));
    }

    // find the first optional token
    let mut first_optional = usize::MAX;
    if !is_host {
        for (i, token) in tokens.iter().enumerate().rev() {
            // variable is optional when it is not important and has a default value
            match token {
                Token::Variable {
                    name, important, ..
                } if !important && route.has_default(name) => first_optional = i,
                _ => break,
            }
        }
    }

    // compute the matching regexp
    let mut regexp = b"{^".to_vec();
    for i in 0..tokens.len() {
        regexp.extend(compute_regexp(&tokens, i, first_optional));
    }
    regexp.extend_from_slice(b"$}sD");
    if is_host {
        regexp.push(b'i');
    }

    // enable Utf8 matching if really required
    if needs_utf8 {
        regexp.push(b'u');
        for token in &mut tokens {
            if let Token::Variable { utf8, .. } = token {
                *utf8 = Some(true);
            }
        }
    }

    let static_prefix = determine_static_prefix(route, &tokens);
    Ok(Compiled {
        static_prefix,
        regex: regexp,
        tokens: PhpArray::list(tokens.iter().rev().map(Token::to_value)),
        variables,
    })
}

/// `determineStaticPrefix()`: the longest static prefix possible for a
/// route.
fn determine_static_prefix(route: &Route, tokens: &[Token]) -> Vec<u8> {
    match tokens.first() {
        None => Vec::new(),
        Some(Token::Variable {
            separator, name, ..
        }) => {
            let separator = separator.clone().unwrap_or_default();
            if route.has_default(name) || separator == b"/" {
                Vec::new()
            } else {
                separator
            }
        }
        Some(Token::Text(text)) => {
            let mut prefix = text.clone();
            if let Some(Token::Variable {
                separator: Some(separator),
                name,
                ..
            }) = tokens.get(1)
                && separator != b"/"
                && !route.has_default(name)
            {
                prefix.extend_from_slice(separator);
            }
            prefix
        }
    }
}

/// `findNextSeparator()`: the next static character in the pattern that
/// will serve as a separator, or the empty string when none available.
fn find_next_separator(
    pattern: &[u8],
    use_utf8: bool,
    separators: &[u8],
) -> Result<Vec<u8>, Error> {
    if pattern.is_empty() {
        return Ok(Vec::new());
    }
    // first remove all placeholders from the pattern so we can find the next real static character
    let pattern = pcre(&PLACEHOLDER)?
        .replace(pattern, b"")
        .map_err(match_error)?;
    if pattern.is_empty() {
        return Ok(Vec::new());
    }
    let first = if use_utf8 {
        // preg_match('/^./u'): `.` is not a newline. Without a match
        // `$pattern[0]` is null, which every string contains, and the null
        // is returned from a function declared to return a string.
        if pattern[0] == b'\n' {
            return Err(Error::new(
                Class::PhpTypeError,
                "Symfony\\Component\\Routing\\RouteCompiler::findNextSeparator(): Return value must be of type string, null returned",
            ));
        }
        first_char(&pattern).to_vec()
    } else {
        pattern[..1].to_vec()
    };
    Ok(if contains(separators, &first) {
        first
    } else {
        Vec::new()
    })
}

/// `computeRegexp()`: the regexp used to match a specific token. It can
/// be static text or a subpattern.
fn compute_regexp(tokens: &[Token], index: usize, first_optional: usize) -> Vec<u8> {
    let (separator, regex, name) = match &tokens[index] {
        // Text tokens
        Token::Text(text) => return preg_quote(text, None),
        Token::Variable {
            separator,
            regex,
            name,
            ..
        } => (separator.as_deref().unwrap_or_default(), regex, name),
    };

    // Variable tokens
    let group = concat(&[
        &preg_quote(separator, None),
        b"(?P<",
        name,
        b">",
        regex,
        b")",
    ]);
    if index == 0 && first_optional == 0 {
        // When the only token is an optional variable token, the separator is required
        return concat(&[&group, b"?"]);
    }
    if index < first_optional {
        return group;
    }
    // Enclose each optional token in a subpattern to make it optional.
    // "?:" means it is non-capturing, i.e. the portion of the subject string that
    // matched the optional subpattern is not passed back.
    let mut regexp = concat(&[b"(?:", &group]);
    let count = tokens.len();
    if count - 1 == index {
        // Close the optional subpatterns
        let n = count - first_optional - usize::from(first_optional == 0);
        regexp.extend(b")?".repeat(n));
    }
    regexp
}

/// `transformCapturingGroupsToNonCapturings()`.
fn transform_capturing_groups_to_non_capturings(regexp: &[u8]) -> Vec<u8> {
    let mut r = regexp.to_vec();
    let mut i = 0;
    while i < r.len() {
        if r[i] == b'\\' {
            i += 2;
            continue;
        }
        if r[i] != b'(' || i + 2 >= r.len() {
            i += 1;
            continue;
        }
        i += 1;
        if r[i] == b'*' || r[i] == b'?' {
            i += 2;
            continue;
        }
        r.splice(i..i, *b"?:");
        i += 2;
    }
    r
}

/// `str_replace($search, $replace, $subject)` for one non-empty search.
fn replace(subject: &[u8], search: &[u8], with: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(subject.len());
    let mut i = 0;
    while i < subject.len() {
        if subject[i..].starts_with(search) {
            out.extend_from_slice(with);
            i += search.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    out
}
