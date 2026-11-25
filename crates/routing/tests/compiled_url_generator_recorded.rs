//! `CompiledUrlGeneratorDumper` and `CompiledUrlGenerator` against PHP 8.4.27
//! running the pinned symfony/routing, recorded by
//! `oracle/recorders/routing_compiled_url_generator.php` (`scripts/record.sh
//! routing-compiled-url-generator`): 1,500 generated collections of routes
//! and aliases (chains, cycles, missing targets, deprecations, localized
//! names, name prefixes), each dumped and read back with `require` in PHP,
//! compared with `compiled()` (the array, the exception, the deprecations
//! triggered), then three `generate()` calls each.
//!
//! Upstream: none

use php_value::{Key, PhpArray, PhpString, Value, vectors};
use serde_json::Value as Json;
use symfony_deprecation_contracts::collect;
use symfony_routing::{
    compiled_url_generator::{CompiledRoutes, CompiledUrlGenerator},
    compiled_url_generator_dumper::CompiledUrlGeneratorDumper,
    exception::Error,
    request_context::RequestContext,
    route::{Arguments, Route},
    route_collection::RouteCollection,
    url_generator::{ReferenceType, UrlGenerator, UrlGeneratorInterface},
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
        match entry[0].as_str() {
            Some("route") => {
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
                routes.add(PhpString::from(bytes(&entry[1])?), route, 0);
            }
            Some("alias") => {
                let alias = routes.add_alias(&bytes(&entry[1])?, &bytes(&entry[2])?)?;
                if let Some(d) = entry[3].as_array() {
                    alias.set_deprecated(bytes(&d[0])?, bytes(&d[1])?, bytes(&d[2])?)?;
                }
            }
            _ => {
                routes.add_name_prefix(&bytes(&entry[1])?)?;
                let mut outer = RouteCollection::new();
                outer.add_collection(routes);
                routes = outer;
            }
        }
    }
    Ok(routes)
}

/// The array `require` returns for these compiled routes.
fn as_array(compiled: &CompiledRoutes) -> PhpArray {
    let mut out = PhpArray::new();
    for (name, route) in compiled.iter() {
        let deprecations = route.deprecations.iter().map(|d| {
            let fields: PhpArray = [
                ("package", Value::String(d.package.clone())),
                ("version", Value::String(d.version.clone())),
                ("message", Value::String(d.message.clone())),
            ]
            .into_iter()
            .collect();
            Value::Array(fields)
        });
        let entry = PhpArray::list([
            Value::Array(route.variables.clone()),
            Value::Array(route.defaults.clone()),
            Value::Array(route.requirements.clone()),
            Value::Array(route.tokens.clone()),
            Value::Array(route.host_tokens.clone()),
            Value::Array(route.schemes.clone()),
            Value::Array(PhpArray::list(deprecations)),
        ]);
        out.insert(Key::from_bytes(name.as_bytes()), Value::Array(entry));
    }
    out
}

fn messages(deprecations: &[symfony_deprecation_contracts::Deprecation]) -> Vec<String> {
    deprecations
        .iter()
        .map(|d| String::from_utf8_lossy(&d.message).into_owned())
        .collect()
}

fn recorded_messages(json: &Json) -> Result<Vec<String>, TestError> {
    json.as_array()
        .ok_or("deprecations")?
        .iter()
        .map(|m| Ok(String::from_utf8_lossy(&bytes(m)?).into_owned()))
        .collect()
}

fn thrown(e: &Error) -> String {
    format!("throws {} {:?}", e.class().name(), e.message())
}

fn recorded_throw(json: &Json) -> Result<String, TestError> {
    Ok(format!(
        "throws {} {:?}",
        json[1].as_str().ok_or("class")?,
        PhpString::from(bytes(&json[2])?)
    ))
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

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one replay loop over the recorded cases"
)]
fn dump_and_generate_match_php() -> Result<(), TestError> {
    let cases: Vec<Json> = serde_json::from_str(include_str!("data/compiled_url_generator.json"))?;
    assert_eq!(cases.len(), 1500);
    let mut failures = Vec::new();
    let (mut dumps, mut calls) = (0, 0);
    for (i, case) in cases.iter().enumerate() {
        let routes = collection(&case[0])?;
        let dump = &case[1];
        let (result, triggered) = collect(|| CompiledUrlGeneratorDumper::new(routes).compiled());
        let recorded = recorded_messages(&dump[if dump[0] == "ok" { 2 } else { 3 }])?;
        if messages(&triggered) != recorded {
            failures.push(format!(
                "case {i} dump deprecations: PHP {recorded:?}, Rust {:?}",
                messages(&triggered)
            ));
        }
        let compiled = match (result, dump[0].as_str()) {
            (Ok(compiled), Some("ok")) => {
                let expected = array(&dump[1])?;
                if as_array(&compiled) != expected {
                    failures.push(format!(
                        "case {i} compiled: {}\n  PHP  {expected:?}\n  Rust {:?}",
                        case[0],
                        as_array(&compiled)
                    ));
                    continue;
                }
                compiled
            }
            (Err(e), Some("throws")) => {
                let want = recorded_throw(dump)?;
                if thrown(&e) != want {
                    failures.push(format!("case {i} dump: PHP {want}, Rust {}", thrown(&e)));
                }
                continue;
            }
            (got, _) => {
                failures.push(format!(
                    "case {i} dump: PHP {}, Rust {:?}",
                    dump[0],
                    got.map(|c| c.len())
                ));
                continue;
            }
        };
        dumps += 1;

        for call in case[2].as_array().ok_or("calls")? {
            let c = &call[0];
            let mut context = RequestContext::new(
                bytes(&c[0])?,
                "GET",
                bytes(&c[1])?,
                bytes(&c[2])?,
                80,
                443,
                "/",
                "",
                None,
            );
            context.set_parameters(array(&c[3])?);
            let default_locale = if call[1].is_null() {
                None
            } else {
                Some(PhpString::from(bytes(&call[1])?))
            };
            let compiled_generator = CompiledUrlGenerator::new(
                compiled.clone(),
                context.clone(),
                None,
                default_locale.clone(),
            );
            // the same call through UrlGenerator on the collection
            let plain_generator =
                UrlGenerator::new(collection(&case[0])?, context, None, default_locale);
            let generators: [(&str, &dyn UrlGeneratorInterface, usize); 2] = [
                ("compiled", &compiled_generator, 5),
                ("plain", &plain_generator, 7),
            ];
            let name = bytes(&call[2])?;
            let reference = reference_type(&call[4])?;
            for (kind, generator, at) in generators {
                let parameters = array(&call[3])?;
                let (result, triggered) =
                    collect(|| generator.generate(&name, parameters, reference));
                let got = match result {
                    Ok(url) => format!("ok {url:?}"),
                    Err(e) => thrown(&e),
                };
                let want = if call[at][0] == "ok" {
                    format!("ok {:?}", PhpString::from(bytes(&call[at][1])?))
                } else {
                    recorded_throw(&call[at])?
                };
                let recorded = recorded_messages(&call[at + 1])?;
                if got != want || messages(&triggered) != recorded {
                    failures.push(format!(
                        "case {i} {kind} call {call}:\n  PHP  {want} {recorded:?}\n  Rust {got} {:?}",
                        messages(&triggered)
                    ));
                }
            }
            calls += 1;
        }
    }
    assert!(
        failures.is_empty(),
        "{} differ ({dumps} dumps, {calls} calls compared):\n{}",
        failures.len(),
        failures[..failures.len().min(6)].join("\n")
    );
    assert!(
        dumps >= 1200 && calls >= 3600,
        "{dumps} dumps, {calls} calls"
    );
    Ok(())
}
