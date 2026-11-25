//! Route, `RouteCompiler`, `CompiledRoute`, `RouteCollection` and Alias against
//! PHP 8.4.27's answers recorded by `oracle/recorders/routing_route.php`
//! (`scripts/record.sh routing-route`):
//!
//! - 2,000 generated routes (paths and hosts with inline settings,
//!   defaults with `_locale`/`_canonical_route`, requirements, `utf8`
//!   options, schemes, methods): the constructed route, `compile()`, the
//!   route after compiling, and `serialize()`'s bytes;
//! - 1,000 generated requirements through `setRequirement()`;
//! - 300 generated `RouteCollection` scripts (add with priorities, aliases,
//!   alias deprecations, remove, addNamePrefix, addPrefix, addCollection):
//!   each step's answer, then `all()`, the aliases, and `get()` of every
//!   name with the deprecations it triggers;
//! - Route and `CompiledRoute` payloads with a value of another type in
//!   each property, through `unserialize()`.
//!
//! Values compare with `===`; a thrown error by class and message.
//!
//! Upstream: none

use php_value::{
    Key, PhpArray, PhpString, Value,
    constants::Registry,
    serialize::unserialize,
    vectors::{self, Error, base64},
};
use serde_json::Value as Json;
use symfony_routing::{
    compiled_route::CompiledRoute,
    exception::Error as RoutingError,
    route::{Arguments, Route},
    route_collection::RouteCollection,
};

type Answer = Result<Value, (String, Vec<u8>)>;

fn data() -> Result<Json, Error> {
    Ok(serde_json::from_str(include_str!("data/route.json"))?)
}

fn recorded(json: &Json) -> Result<Answer, Error> {
    if json[0] == "throws" {
        let class = json[1].as_str().ok_or("class")?.to_owned();
        let message = base64(json[2].as_str().ok_or("message")?)?;
        return Ok(Err((class, message)));
    }
    Ok(Ok(vectors::decode(json)?))
}

fn thrown(e: &RoutingError) -> (String, Vec<u8>) {
    (e.class().name().to_owned(), e.message().to_vec())
}

fn answer(result: Result<Value, RoutingError>) -> Answer {
    result.map_err(|e| thrown(&e))
}

fn array(pairs: Vec<(&str, Value)>) -> Value {
    Value::Array(pairs.into_iter().collect())
}

fn route_state(r: &Route) -> Value {
    array(vec![
        ("path", Value::String(r.path().clone())),
        ("host", Value::String(r.host().clone())),
        ("defaults", Value::Array(r.defaults().clone())),
        ("requirements", Value::Array(r.requirements().clone())),
        ("options", Value::Array(r.options().clone())),
        ("schemes", Value::Array(r.schemes().clone())),
        ("methods", Value::Array(r.methods().clone())),
        ("condition", Value::String(r.condition().clone())),
    ])
}

fn compiled_state(c: &CompiledRoute) -> Value {
    Value::Array(PhpArray::list([
        Value::String(c.static_prefix().clone()),
        Value::String(c.regex().clone()),
        Value::Array(c.tokens().clone()),
        Value::Array(c.path_variables().clone()),
        c.host_regex().cloned().map_or(Value::Null, Value::String),
        Value::Array(c.host_tokens().clone()),
        Value::Array(c.host_variables().clone()),
        Value::Array(c.variables().clone()),
    ]))
}

fn strings(value: &Value) -> Result<Vec<PhpString>, Error> {
    value
        .as_array()
        .ok_or("expected a list")?
        .values()
        .map(|v| {
            v.as_string()
                .cloned()
                .ok_or_else(|| "expected a string".into())
        })
        .collect()
}

fn item(list: &PhpArray, i: i64) -> Result<&Value, Error> {
    list.get(&Key::Int(i))
        .ok_or_else(|| format!("no item {i}").into())
}

fn arr(value: &Value) -> Result<PhpArray, Error> {
    value
        .as_array()
        .cloned()
        .ok_or_else(|| "expected an array".into())
}

fn bytes(value: &Value) -> Result<Vec<u8>, Error> {
    Ok(value.as_string().ok_or("expected a string")?.to_vec())
}

#[test]
fn recorded_routes() -> Result<(), Error> {
    let data = data()?;
    let routes = data["routes"].as_array().ok_or("routes")?;
    assert_eq!(2000, routes.len());
    for (n, case) in routes.iter().enumerate() {
        let input = vectors::decode(&case[0])?;
        let input = input.as_array().ok_or("input")?;
        let path = bytes(item(input, 0)?)?;
        let arguments = Arguments {
            defaults: arr(item(input, 1)?)?,
            requirements: arr(item(input, 2)?)?,
            options: arr(item(input, 3)?)?,
            host: PhpString::from(bytes(item(input, 4)?)?),
            schemes: strings(item(input, 5)?)?,
            methods: strings(item(input, 6)?)?,
            condition: PhpString::default(),
        };
        let route = Route::with(path.as_slice(), arguments);
        let constructed = answer(route.as_ref().map(route_state).map_err(Clone::clone));
        assert_eq!(
            recorded(&case[1])?,
            constructed,
            "route {n}: construct {input:?}"
        );
        let Ok(mut route) = route else { continue };

        let compiled = answer(route.compile().map(compiled_state));
        assert_eq!(
            recorded(&case[2])?,
            compiled,
            "route {n}: compile {input:?}"
        );
        assert_eq!(
            vectors::decode(&case[3])?,
            route_state(&route),
            "route {n}: after compile {input:?}"
        );
        let serialized = route.serialize().map_err(|e| e.to_string())?;
        assert_eq!(
            base64(case[4].as_str().ok_or("serialized")?)?,
            serialized.to_vec(),
            "route {n}: serialize {input:?}"
        );
    }
    Ok(())
}

#[test]
fn recorded_requirements() -> Result<(), Error> {
    let data = data()?;
    let cases = data["requirements"].as_array().ok_or("requirements")?;
    assert_eq!(1000, cases.len());
    for case in cases {
        let requirement = base64(case[0].as_str().ok_or("requirement")?)?;
        let actual = answer((|| {
            let mut route = Route::new("/")?;
            Ok(route
                .set_requirement(b"k", &requirement)?
                .requirement(b"k")
                .cloned()
                .map_or(Value::Null, Value::String))
        })());
        assert_eq!(
            recorded(&case[1])?,
            actual,
            "{:?}",
            PhpString::from(requirement.clone())
        );
    }
    Ok(())
}

/// A name as a PHP array key gives it back (`"0"` is the integer 0).
fn key_value(name: &[u8]) -> Value {
    Key::from_bytes(name).to_value()
}

fn optional(value: &Value) -> Option<PhpString> {
    value.as_string().cloned()
}

/// The recorder's `apply()`.
fn apply(c: &mut RouteCollection, step: &PhpArray) -> Result<Value, RoutingError> {
    let text = |i: i64| -> Vec<u8> {
        step.get(&Key::Int(i))
            .and_then(Value::as_string)
            .map(|s| s.to_vec())
            .unwrap_or_default()
    };
    let kind = text(0);
    match kind.as_slice() {
        b"add" => {
            let mut route = Route::new(text(2))?;
            if let Some(canonical) = step.get(&Key::Int(4)).and_then(optional) {
                route.set_default(b"_canonical_route", Value::String(canonical))?;
            }
            let priority = match step.get(&Key::Int(3)) {
                Some(Value::Int(p)) => *p,
                _ => 0,
            };
            c.add(text(1), route, priority);
            Ok(Value::Null)
        }
        b"alias" => {
            c.add_alias(&text(1), &text(2))?;
            Ok(Value::Null)
        }
        b"deprecate" => {
            let Some(alias) = c.alias_mut(&text(1)) else {
                return Ok(Value::string("no alias"));
            };
            alias.set_deprecated(text(3), text(4), text(2))?;
            let deprecation = alias.deprecation(b"NAME");
            Ok(deprecation.map_or(Value::Null, |d| {
                array(vec![
                    ("package", Value::String(d.package)),
                    ("version", Value::String(d.version)),
                    ("message", Value::String(d.message)),
                ])
            }))
        }
        b"remove" => {
            let names: Vec<Vec<u8>> = step
                .get(&Key::Int(1))
                .and_then(Value::as_array)
                .map(|a| {
                    a.values()
                        .filter_map(Value::as_string)
                        .map(|s| s.to_vec())
                        .collect()
                })
                .unwrap_or_default();
            if names.len() == 1 {
                c.remove(&names[0]);
            } else {
                let refs: Vec<&[u8]> = names.iter().map(Vec::as_slice).collect();
                c.remove_all(&refs);
            }
            Ok(Value::Null)
        }
        b"name_prefix" => {
            c.add_name_prefix(&text(1))?;
            Ok(Value::Null)
        }
        b"prefix" => {
            let defaults: PhpArray = [("d", Value::string("v"))].into_iter().collect();
            c.add_prefix(&text(1), &defaults, &PhpArray::new())?;
            Ok(Value::Null)
        }
        b"collection" => {
            let mut sub = RouteCollection::new();
            for s in step
                .get(&Key::Int(1))
                .and_then(Value::as_array)
                .into_iter()
                .flat_map(PhpArray::values)
            {
                if let Some(s) = s.as_array() {
                    apply(&mut sub, s)?;
                }
            }
            c.add_collection(sub);
            Ok(Value::Null)
        }
        other => panic!("unknown step {}", String::from_utf8_lossy(other)),
    }
}

type State = (Value, Value, Vec<(Answer, Vec<Vec<u8>>)>);

/// The recorder's `collection_state()`.
fn collection_state(c: &RouteCollection, probe: &[&str]) -> State {
    let mut all = Vec::new();
    for (name, route) in c.all() {
        all.push(Value::Array(PhpArray::list([
            key_value(name),
            Value::String(route.path().clone()),
            c.priority(name).map_or(Value::Null, Value::Int),
            route
                .default_value(b"_canonical_route")
                .cloned()
                .unwrap_or_default(),
        ])));
    }
    let aliases: Vec<Value> = c
        .aliases()
        .iter()
        .map(|(name, alias)| {
            Value::Array(PhpArray::list([
                key_value(name),
                Value::String(alias.id().clone()),
                Value::Bool(alias.is_deprecated()),
            ]))
        })
        .collect();
    let gets = probe
        .iter()
        .map(|name| {
            let notices = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let sink = notices.clone();
            let got = symfony_deprecation_contracts::with_handler(
                move |d| sink.borrow_mut().push(d.message.clone()),
                || c.get(name.as_bytes()).map(|r| r.map(|r| r.path().clone())),
            );
            let got = answer(got.map(|r| r.map_or(Value::Null, Value::String)));
            let notices = notices.borrow().clone();
            (got, notices)
        })
        .collect();
    (
        Value::Array(PhpArray::list(all)),
        Value::Array(PhpArray::list(aliases)),
        gets,
    )
}

/// A typed value that went through `typed_value()` a second time, back
/// to its JSON form.
fn to_json(value: &Value) -> Result<Json, Error> {
    Ok(match value {
        Value::Null => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Int(i) => Json::from(*i),
        Value::String(s) => Json::String(String::from_utf8(s.to_vec())?),
        Value::Array(a) => Json::Array(a.values().map(to_json).collect::<Result<_, _>>()?),
        other => return Err(format!("unexpected {other:?}").into()),
    })
}

fn recorded_state(json: &Json) -> Result<State, Error> {
    let state = vectors::decode(json)?;
    let state = state.as_array().ok_or("state")?;
    let gets = item(state, 2)?
        .as_array()
        .ok_or("gets")?
        .values()
        .map(|g| -> Result<_, Error> {
            let g = g.as_array().ok_or("get")?;
            let got = recorded(&to_json(item(g, 0)?)?)?;
            let notices = item(g, 1)?
                .as_array()
                .ok_or("notices")?
                .values()
                .map(bytes)
                .collect::<Result<_, _>>()?;
            Ok((got, notices))
        })
        .collect::<Result<_, _>>()?;
    Ok((item(state, 0)?.clone(), item(state, 1)?.clone(), gets))
}

#[test]
fn recorded_collections() -> Result<(), Error> {
    const PROBE: [&str; 7] = ["a", "b", "c", "p_a", "0", "1", "é"];
    let data = data()?;
    let scripts = data["collections"].as_array().ok_or("collections")?;
    assert_eq!(300, scripts.len());
    for (n, script) in scripts.iter().enumerate() {
        let mut c = RouteCollection::new();
        for (i, step) in script.as_array().ok_or("script")?.iter().enumerate() {
            let input = vectors::decode(&step[0])?;
            let actual = answer(apply(&mut c, input.as_array().ok_or("step")?));
            assert_eq!(
                recorded(&step[1])?,
                actual,
                "script {n} step {i}: {input:?}"
            );
            assert_eq!(
                recorded_state(&step[2])?,
                collection_state(&c, &PROBE),
                "script {n} step {i}: {input:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn recorded_unserialize() -> Result<(), Error> {
    let data = data()?;
    let cases = data["unserialize"].as_array().ok_or("unserialize")?;
    assert_eq!(131, cases.len());
    let mut classes = Route::classes();
    classes.allow("stdClass");
    for case in cases {
        let payload = base64(case[1].as_str().ok_or("payload")?)?;
        let actual = if case[0] == "route" {
            match Route::unserialize(&payload, &classes, &[]) {
                Ok(Some(route)) => Ok(route_state(&route)),
                Ok(None) => return Err("not a route".into()),
                Err(e) => Err(thrown(&e)),
            }
        } else {
            let Some(Value::Object(o)) = unserialize(&payload, &classes, &Registry::empty()) else {
                return Err("not an object".into());
            };
            answer(
                CompiledRoute::from_serialized(o.class(), &o.properties())
                    .map(|c| compiled_state(&c)),
            )
        };
        assert_eq!(
            recorded(&case[2])?,
            actual,
            "{}",
            String::from_utf8_lossy(&payload)
        );
    }
    Ok(())
}
