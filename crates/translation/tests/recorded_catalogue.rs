//! Replay the pinned PHP catalogue operation vectors, including ordered
//! arrays, exception classes/text, partial results and lazy cache identity.
//!
//! Upstream: none
use php_value::{Key, PhpArray, Value, vectors::decode};
use serde_json::Value as Json;
use symfony_translation::catalogue::{AbstractOperation, MergeOperation, TargetOperation};
use symfony_translation::{Error, MessageCatalogue};

type BoxError = Box<dyn std::error::Error + Send + Sync>;
fn array(value: &Value) -> Result<&PhpArray, BoxError> {
    value.as_array().ok_or_else(|| "expected array".into())
}
fn item(value: &Value, index: i64) -> Result<&Value, BoxError> {
    array(value)?
        .get(&Key::Int(index))
        .ok_or_else(|| "missing item".into())
}
fn bytes(value: &Value) -> Result<&[u8], BoxError> {
    match value {
        Value::String(s) => Ok(s.as_bytes()),
        _ => Err("expected string".into()),
    }
}
fn key_bytes(key: &Key) -> Vec<u8> {
    match key {
        Key::Int(i) => i.to_string().into_bytes(),
        Key::Str(s) => s.as_bytes().to_vec(),
    }
}
fn catalogue(spec: &Value) -> Result<MessageCatalogue, BoxError> {
    let c = MessageCatalogue::new(bytes(item(spec, 0)?)?, array(item(spec, 1)?)?.clone());
    for (index, catalogue_metadata) in [(2, false), (3, true)] {
        for (domain, pairs) in array(item(spec, index)?)?.iter() {
            for (id, value) in array(pairs)?.iter() {
                if catalogue_metadata {
                    c.set_catalogue_metadata(&key_bytes(id), value.clone(), &key_bytes(domain));
                } else {
                    c.set_metadata(&key_bytes(id), value.clone(), &key_bytes(domain));
                }
            }
        }
    }
    let fallback = item(spec, 4)?;
    if !fallback.is_null() {
        c.add_fallback_catalogue(&catalogue(fallback)?)?;
    }
    Ok(c)
}
fn snapshot(c: &MessageCatalogue) -> Value {
    let (messages, metadata, catalogue_metadata) = c.raw();
    Value::Array(PhpArray::list([
        Value::Array(messages),
        Value::Array(metadata),
        Value::Array(catalogue_metadata),
        Value::Array(PhpArray::list(
            c.get_resources()
                .iter()
                .map(|r| Value::string(r.to_string())),
        )),
        Value::Bool(c.get_fallback_catalogue().is_some()),
    ]))
}
fn text(value: &Json) -> Result<&str, BoxError> {
    value.as_str().ok_or_else(|| "expected JSON string".into())
}
fn compare(expected: &Json, got: Result<Value, Error>, label: &str) -> Result<(), BoxError> {
    if expected[0] == "throws" {
        let e = got
            .err()
            .ok_or_else(|| format!("{label}: expected exception {expected}"))?;
        assert_eq!(e.class().name(), text(&expected[1])?, "{label}");
        assert_eq!(e.get_message(), text(&expected[2])?.as_bytes(), "{label}");
        assert_eq!(e.get_code(), 0, "{label}");
    } else {
        assert_eq!(got?, decode(expected)?, "{label}");
    }
    Ok(())
}
#[test]
fn recorded_catalogue_operations() -> Result<(), BoxError> {
    replay(include_str!("data/catalogue.json"), true, 524)?;
    replay(include_str!("data/catalogue_no_intl.json"), false, 4)
}

fn replay(recorded: &str, intl: bool, count: usize) -> Result<(), BoxError> {
    let data: Json = serde_json::from_str(recorded)?;
    assert_eq!(data["php"], "8.4.27");
    assert_eq!(data["intl"], intl);
    let cases = data["cases"].as_array().ok_or("cases")?;
    assert_eq!(cases.len(), count);
    let mut calls = 0;
    for (index, case) in cases.iter().enumerate() {
        let source = catalogue(&decode(&case[1])?)?;
        let target = catalogue(&decode(&case[2])?)?;
        enum Operation {
            Merge(MergeOperation),
            Target(TargetOperation),
        }
        let operation = match text(&case[0])? {
            "merge" => MergeOperation::new(source.clone(), target.clone()).map(Operation::Merge),
            "target" => TargetOperation::new(source.clone(), target.clone()).map(Operation::Target),
            _ => return Err("unknown operation".into()),
        };
        let mut operation = match operation {
            Ok(op) => op,
            Err(e) => {
                compare(&case[5][0], Err(e), &format!("case {index} constructor"))?;
                calls += 1;
                continue;
            }
        };
        let op: &mut AbstractOperation = match &mut operation {
            Operation::Merge(op) => op,
            Operation::Target(op) => op,
        };
        let steps = case[3].as_array().ok_or("steps")?;
        assert_eq!(steps.len(), case[5].as_array().ok_or("outcomes")?.len());
        for (i, step) in steps.iter().enumerate() {
            let arg = |n| -> Result<&[u8], BoxError> { Ok(text(&step[n])?.as_bytes()) };
            let got = match text(&step[0])? {
                "domains" => Ok(Value::Array(op.get_domains())),
                "all" => op.get_messages(arg(1)?).map(Value::Array),
                "new" => op.get_new_messages(arg(1)?).map(Value::Array),
                "obsolete" => op.get_obsolete_messages(arg(1)?).map(Value::Array),
                "result" => Ok(snapshot(&op.get_result())),
                "move" => op
                    .move_messages_to_intl_domains_with_batch(
                        arg(1)?,
                        case[4].as_bool().ok_or("intl")?,
                    )
                    .map(|()| Value::Null),
                "source" => {
                    source.set(arg(1)?, arg(2)?, arg(3)?);
                    Ok(Value::Null)
                }
                "target" => {
                    target.set(arg(1)?, arg(2)?, arg(3)?);
                    Ok(Value::Null)
                }
                "result_set" => {
                    op.get_result().set(arg(1)?, arg(2)?, arg(3)?);
                    Ok(Value::Null)
                }
                _ => return Err("unknown action".into()),
            };
            compare(
                &case[5][i],
                got,
                &format!("case {index} {} step {i}: {step}", text(&case[0])?),
            )?;
            calls += 1;
        }
    }
    println!("{} catalogue cases, {calls} calls matched PHP", cases.len());
    Ok(())
}
