//! UrlGenerator against PHP 8.4.27 running the pinned symfony/routing,
//! recorded by oracle/recorders/routing_url_generator.php
//! (`scripts/record.sh routing-url-generator`): 3,000 generated
//! collections, contexts and `generate()` calls (URL, exception class and
//! message, and what the logger received), 2,000 `getRelativePath()`
//! calls, and the GHSA-72xp-p242-47p9 check.
//!
//! Upstream: none

use std::sync::{Arc, Mutex};

use php_value::{PhpArray, PhpString, Value, vectors};
use psr_log::{Context, ContextValue, LogLevel, LoggerInterface};
use serde_json::Value as Json;
use symfony_routing::{
    exception::Error,
    request_context::RequestContext,
    route::{Arguments, Route},
    route_collection::RouteCollection,
    url_generator::{
        ConfigurableRequirementsInterface, ReferenceType, UrlGenerator, UrlGeneratorInterface,
    },
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

fn bytes(json: &Json) -> Result<Vec<u8>, TestError> {
    vectors::base64(json.as_str().ok_or("expected base64")?)
}

fn array(json: &Json) -> Result<PhpArray, TestError> {
    match vectors::decode(json)? {
        Value::Array(a) => Ok(a),
        other => Err(format!("expected an array, got {other:?}").into()),
    }
}

fn collection(entries: &Json) -> Result<RouteCollection, TestError> {
    let mut routes = RouteCollection::new();
    for entry in entries.as_array().ok_or("entries")? {
        let name = bytes(&entry[1])?;
        if entry[0] == "alias" {
            routes.add_alias(&name, &bytes(&entry[2])?)?;
            continue;
        }
        let schemes = entry[7]
            .as_array()
            .ok_or("schemes")?
            .iter()
            .map(|s| bytes(s).map(PhpString::from))
            .collect::<Result<_, _>>()?;
        let route = Route::with(
            bytes(&entry[2])?,
            Arguments {
                defaults: array(&entry[3])?,
                requirements: array(&entry[4])?,
                options: array(&entry[5])?,
                host: PhpString::from(bytes(&entry[6])?),
                schemes,
                ..Arguments::default()
            },
        )?;
        routes.add(PhpString::from(name), route, 0);
    }
    Ok(routes)
}

/// Keeps `[level, message, given]` as the recorder's logger does.
#[derive(Default)]
struct Recorder(Mutex<Vec<(String, String, String)>>);

impl LoggerInterface for Recorder {
    fn log(&self, level: LogLevel, message: &str, context: Context) {
        let given = context
            .iter()
            .find(|(k, _)| k == "given")
            .map(|(_, v)| match v {
                ContextValue::String(s) => s.clone(),
                ContextValue::Exception(e) => e.to_string(),
            })
            .unwrap_or_default();
        if let Ok(mut records) = self.0.lock() {
            records.push((level.as_str().to_owned(), message.to_owned(), given));
        }
    }
}

fn reference_type(json: &Json) -> Result<ReferenceType, TestError> {
    Ok(match json.as_i64() {
        Some(0) => ReferenceType::AbsoluteUrl,
        Some(1) => ReferenceType::AbsolutePath,
        Some(2) => ReferenceType::RelativePath,
        Some(3) => ReferenceType::NetworkPath,
        _ => return Err("reference type".into()),
    })
}

fn outcome(result: Result<PhpString, Error>, logger: &Recorder) -> Result<String, TestError> {
    Ok(match result {
        Ok(url) => {
            let records = logger.0.lock().map_err(|e| e.to_string())?;
            format!("ok {:?} {:?}", url, *records)
        }
        Err(e) => format!("throws {} {:?}", e.class().name(), e.message()),
    })
}

fn expected(result: &Json) -> Result<String, TestError> {
    Ok(if result[0] == "ok" {
        let lossy = |j: &Json| -> Result<String, TestError> {
            Ok(String::from_utf8_lossy(&bytes(j)?).into_owned())
        };
        let mut records = Vec::new();
        for r in result[2].as_array().ok_or("records")? {
            records.push((
                r[0].as_str().ok_or("level")?.to_owned(),
                lossy(&r[1])?,
                lossy(&r[2])?,
            ));
        }
        format!("ok {:?} {:?}", PhpString::from(bytes(&result[1])?), records)
    } else {
        format!(
            "throws {} {:?}",
            result[1].as_str().ok_or("class")?,
            PhpString::from(bytes(&result[2])?)
        )
    })
}

fn data() -> Result<Json, TestError> {
    Ok(serde_json::from_str(include_str!(
        "data/url_generator.json"
    ))?)
}

#[test]
fn generate_matches_php() -> Result<(), TestError> {
    let data = data()?;
    let cases = data["generate"].as_array().ok_or("generate")?;
    assert_eq!(cases.len(), 3000);
    let mut failures = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        let routes = collection(&case[0])?;
        let c = &case[1];
        let context = RequestContext::new(
            bytes(&c[0])?,
            "GET",
            bytes(&c[1])?,
            bytes(&c[2])?,
            c[3].as_i64().ok_or("http port")?,
            c[4].as_i64().ok_or("https port")?,
            bytes(&c[5])?,
            "",
            Some(array(&c[6])?),
        );
        let logger = Arc::new(Recorder::default());
        let default_locale = if case[3].is_null() {
            None
        } else {
            Some(PhpString::from(bytes(&case[3])?))
        };
        let mut generator =
            UrlGenerator::new(routes, context, Some(logger.clone()), default_locale);
        generator.set_strict_requirements(case[2].as_bool());
        let result = generator.generate(
            &bytes(&case[4])?,
            array(&case[5])?,
            reference_type(&case[6])?,
        );
        let got = outcome(result, &logger)?;
        let want = expected(&case[7])?;
        if got != want {
            failures.push(format!("case {i}: {case}\n  PHP  {want}\n  Rust {got}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(8)].join("\n")
    );
    Ok(())
}

#[test]
fn get_relative_path_matches_php() -> Result<(), TestError> {
    let data = data()?;
    let cases = data["relative"].as_array().ok_or("relative")?;
    assert_eq!(cases.len(), 2000);
    for case in cases {
        let (base, target) = (bytes(&case[0])?, bytes(&case[1])?);
        let got = UrlGenerator::get_relative_path(&base, &target);
        assert_eq!(
            String::from_utf8_lossy(&got),
            String::from_utf8_lossy(&bytes(&case[2])?),
            "{} -> {}",
            String::from_utf8_lossy(&base),
            String::from_utf8_lossy(&target)
        );
    }
    Ok(())
}

/// GHSA-72xp-p242-47p9: PHP's pre-fix check (`#^en|fr|vi|de$#i`) accepts
/// `/evil.com`; the pinned code, and this port, throw.
#[test]
fn ghsa_72xp_pre_fix_check_accepts_the_test_input() -> Result<(), TestError> {
    let data = data()?;
    assert_eq!(data["advisory"]["pre_fix_matches"], true);
    assert_eq!(
        data["advisory"]["pinned"],
        "Symfony\\Component\\Routing\\Exception\\InvalidParameterException"
    );
    Ok(())
}
