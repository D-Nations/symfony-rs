//! `StreamedJsonResponse` against PHP 8.4.27 running the pinned
//! symfony/http-foundation (v8.1.11), recorded by
//! `oracle/recorders/http_foundation_streamed_json.php` (`scripts/record.sh
//! http-foundation-streamed-json`): 1,500 generated scripts. Each builds
//! data from a seeded spec: arrays, scalars, generators (keys of every
//! type; some echo before each yield, some throw), `ArrayObject`, an
//! `IteratorAggregate`, `JsonSerializable` objects (also `Traversable`, or
//! throwing) and `stdClass` objects, with encoding options and headers
//! from pools. Then `send()` or the callback is called once or twice.
//! Compared: the constructor's exception, each call's result, each
//! non-empty echo as a chunk of its own (so a generator's echo must land
//! where PHP's did), then the headers.
//!
//! Upstream: none

use std::sync::Arc;

use php_value::{
    PhpArray, PhpObject, PhpString, Thrown, Value, clock,
    object::{ArrayObject, StdClass},
    vectors,
};
use serde_json::{Value as Json, json};
use symfony_http_foundation::{
    exception::Error,
    generator::{Generator, Yield},
    header_bag::HeaderValues,
    json_response::JsonResponseError,
    php::{Globals, OutputCall, set_globals, with_output},
    response::Response,
    streamed_json_response,
    streamed_response::{Stream, Throwable, echo},
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/streamed_json.json");

fn text(json: &Json) -> Result<&str, TestError> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn bytes(json: &Json) -> Result<PhpString, TestError> {
    Ok(PhpString::from(vectors::base64(text(json)?)?))
}

fn items(json: &Json) -> Result<&Vec<Json>, TestError> {
    json.as_array().ok_or_else(|| "expected a list".into())
}

/// The recorder's `$array($pairs)`.
fn array(pairs: &Json) -> Result<PhpArray, TestError> {
    let mut array = PhpArray::new();
    for pair in items(pairs)? {
        let value = build(&pair[1])?;
        match text(&pair[0][0])? {
            "i" => array.insert(pair[0][1].as_i64().ok_or("int key")?, value),
            _ => array.insert(bytes(&pair[0][1])?, value),
        }
    }
    Ok(array)
}

/// The recorder's anonymous `IteratorAggregate` and `JsonSerializable`
/// classes.
#[derive(Debug)]
enum Anonymous {
    Aggregate(PhpArray),
    Serializable(Value),
    SerializableTraversable(Value),
    Throwing,
}

impl PhpObject for Anonymous {
    fn class(&self) -> &str {
        match self {
            Self::Aggregate(_) => "IteratorAggregate@anonymous",
            _ => "JsonSerializable@anonymous",
        }
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }

    fn json_serialize(&self) -> Option<Result<Value, Thrown>> {
        match self {
            Self::Aggregate(_) => None,
            Self::Serializable(value) | Self::SerializableTraversable(value) => {
                Some(Ok(value.clone()))
            }
            Self::Throwing => Some(Err(Thrown::new("DomainException", "jsonSerialize", 0))),
        }
    }

    fn instance_of(&self, class: &str) -> bool {
        let traversable = matches!(self, Self::Aggregate(_) | Self::SerializableTraversable(_));
        match class {
            "Traversable" | "IteratorAggregate" => traversable,
            "JsonSerializable" => !matches!(self, Self::Aggregate(_)),
            _ => false,
        }
    }

    fn traverse(&self) -> Option<Result<Vec<(Value, Value)>, Thrown>> {
        match self {
            Self::Aggregate(items) => Some(Ok(items
                .iter()
                .map(|(k, v)| (k.clone().into(), v.clone()))
                .collect())),
            Self::SerializableTraversable(_) => {
                Some(Ok(vec![(Value::Int(0), Value::string("ignored"))]))
            }
            _ => None,
        }
    }
}

/// The recorder's generator: item by item, as PHP runs its body.
struct Body {
    items: Vec<Json>,
    echoes: bool,
    throw_at: Option<usize>,
    next: usize,
    /// PHP's largest integer key used so far, which a `yield` without a
    /// key counts on from.
    largest: i64,
    done: bool,
}

impl Body {
    fn item(&mut self) -> Result<Option<(Value, Value)>, TestError> {
        let i = self.next;
        if self.throw_at == Some(i) {
            self.done = true;
            let message = if i == self.items.len() {
                "generator end".to_owned()
            } else {
                format!("generator item {i}")
            };
            return Err(Box::new(Thrown::new("RuntimeException", message, 0)));
        }
        let Some(item) = self.items.get(i).cloned() else {
            self.done = true;
            return Ok(None);
        };
        self.next += 1;
        if self.echoes {
            echo("<y>");
        }
        let key = if item[0].is_null() {
            self.largest += 1;
            Value::Int(self.largest)
        } else {
            let key = build(&item[0])?;
            if let Value::Int(k) = key
                && k > self.largest
            {
                self.largest = k;
            }
            key
        };
        Ok(Some((key, build(&item[1])?)))
    }
}

impl Iterator for Body {
    type Item = Yield;

    fn next(&mut self) -> Option<Yield> {
        if self.done {
            return None;
        }
        match self.item() {
            Ok(pair) => pair.map(Ok),
            Err(error) => Some(Err(match error.downcast::<Thrown>() {
                Ok(thrown) => *thrown,
                Err(other) => Thrown::new("TestError", other.to_string(), 0),
            })),
        }
    }
}

/// The recorder's `$build($spec)`.
fn build(spec: &Json) -> Result<Value, TestError> {
    let object = |o: Anonymous| Value::Object(Arc::new(o));
    Ok(match text(&spec[0])? {
        "null" => Value::Null,
        "bool" => Value::Bool(spec[1].as_bool().ok_or("bool")?),
        "int" => Value::Int(spec[1].as_i64().ok_or("int")?),
        "float" => Value::Float(text(&spec[1])?.parse()?),
        "str" => Value::String(bytes(&spec[1])?),
        "arr" => Value::Array(array(&spec[1])?),
        "gen" => Generator::new(Body {
            items: items(&spec[1])?.clone(),
            echoes: spec[2].as_bool().ok_or("echoes")?,
            throw_at: spec[3].as_u64().and_then(|i| usize::try_from(i).ok()),
            next: 0,
            largest: -1,
            done: false,
        })
        .object(),
        "ao" => Value::Object(Arc::new(ArrayObject::new(array(&spec[1])?))),
        "agg" => object(Anonymous::Aggregate(PhpArray::list(
            items(&spec[1])?
                .iter()
                .map(build)
                .collect::<Result<Vec<_>, _>>()?,
        ))),
        "ser" => object(Anonymous::Serializable(build(&spec[1])?)),
        "sertrav" => object(Anonymous::SerializableTraversable(build(&spec[1])?)),
        "serthrow" => object(Anonymous::Throwing),
        "obj" => {
            let mut properties = PhpArray::new();
            for pair in items(&spec[1])? {
                properties.insert(bytes(&pair[0])?, build(&pair[1])?);
            }
            StdClass::object(properties)
        }
        other => return Err(format!("unknown spec {other}").into()),
    })
}

/// A throwable's class and message.
fn thrown(error: &Throwable) -> Result<(String, PhpString), TestError> {
    if let Some(thrown) = error.downcast_ref::<Thrown>() {
        return Ok((thrown.class.clone(), thrown.message.clone()));
    }
    if let Some(error) = error.downcast_ref::<Error>() {
        return Ok((error.class().name().to_owned(), error.message().clone()));
    }
    if let Some(JsonResponseError::JsonWithPrevious { exception, .. }) =
        error.downcast_ref::<JsonResponseError>()
    {
        return Ok((
            exception.class().name().to_owned(),
            exception.message().clone(),
        ));
    }
    Err(format!("unknown throwable {error}").into())
}

fn same_thrown(recorded: &Json, class: &str, message: &PhpString) -> Result<bool, TestError> {
    Ok(recorded[0] == "throws"
        && recorded[1] == class
        && bytes(&recorded[2])?.as_bytes() == message.as_bytes())
}

fn headers_value(response: &Response) -> Value {
    let mut headers = PhpArray::new();
    for (name, values) in response.headers().all_preserve_case().iter() {
        headers.insert(
            name.clone(),
            Value::Array(PhpArray::list(
                values
                    .iter()
                    .map(|v| v.clone().map_or(Value::Null, Value::String)),
            )),
        );
    }
    Value::Array(headers)
}

/// Each non-empty echo, as the recorder's output buffer of chunk size 1
/// receives them.
fn chunks() -> Vec<Vec<u8>> {
    with_output(|o| {
        o.calls
            .iter()
            .filter_map(|call| match call {
                OutputCall::Echo(text) if !text.as_bytes().is_empty() => {
                    Some(text.as_bytes().to_vec())
                }
                _ => None,
            })
            .collect()
    })
}

#[test]
fn streamed_json_answers_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let scripts = items(&data["scripts"])?;
    assert_eq!(1500, scripts.len());
    let _clock = clock::mock(1_700_000_000);
    let mut failures = Vec::new();
    let mut answers = 0;

    for (n, script) in scripts.iter().enumerate() {
        set_globals(Globals::default());
        let mut headers: Vec<(PhpString, HeaderValues)> = Vec::new();
        for pair in items(&script["headers_in"])? {
            headers.push((
                PhpString::from(text(&pair[0])?),
                HeaderValues::from(text(&pair[1])?),
            ));
        }
        answers += 1;
        let created = streamed_json_response::new(
            build(&script["data"])?,
            200,
            headers.iter().map(|(k, v)| (k.as_bytes(), v.clone())),
            script["options"].as_i64().ok_or("options")?,
        );
        let mut response = match (created, &script["new"]) {
            (Ok(response), Json::Null) => response,
            (Err(error), recorded)
                if same_thrown(recorded, error.class().name(), error.message())? =>
            {
                continue;
            }
            (got, recorded) => {
                failures.push(format!(
                    "script {n} new: expected {recorded}, got {:?}",
                    got.err()
                ));
                continue;
            }
        };

        let ops = items(&script["ops"])?;
        let results = items(&script["results"])?;
        for (i, (op, result)) in ops.iter().zip(results).enumerate() {
            with_output(|o| o.calls.clear());
            let outcome = if text(op)? == "send" {
                response.send(true).map(|_| json!(["self"]))
            } else {
                let callback = response
                    .stream()
                    .and_then(Stream::get_callback)
                    .ok_or("no callback")?;
                callback().map(|()| json!(["null"]))
            };
            answers += 1;
            let same = match &outcome {
                Ok(_) if result[0][0] == "throws" => false,
                Ok(this) if *this == json!(["self"]) => result[0] == *this,
                Ok(_) => vectors::decode(&result[0])?.identical(&Value::Null),
                Err(error) => {
                    let (class, message) = thrown(error)?;
                    same_thrown(&result[0], &class, &message)?
                }
            };
            let got = chunks();
            let expected = items(&result[1])?
                .iter()
                .map(|c| vectors::base64(text(c)?))
                .collect::<Result<Vec<_>, _>>()?;
            if !same || got != expected {
                let show = |c: &[Vec<u8>]| {
                    c.iter()
                        .map(|c| String::from_utf8_lossy(c).into_owned())
                        .collect::<Vec<_>>()
                };
                failures.push(format!(
                    "script {n} op {i} {op}: expected {} / {:?}, got {:?} / {:?}",
                    result[0],
                    show(&expected),
                    outcome.as_ref().map_err(|e| thrown(e).map(|t| t.0).ok()),
                    show(&got),
                ));
            }
        }

        answers += 1;
        if !vectors::decode(&script["headers"])?.identical(&headers_value(&response)) {
            failures.push(format!(
                "script {n} headers: expected {}",
                script["headers"]
            ));
        }
    }
    let total = failures.len();
    failures.truncate(15);
    assert!(
        failures.is_empty(),
        "{total} of {answers} answers differ:\n{}",
        failures.join("\n")
    );
    eprintln!("{answers} answers");
    Ok(())
}
