//! `Session`, its bags and the mock storages against PHP 8.4.27 running the
//! pinned symfony/http-foundation (v8.1.11), recorded by
//! `oracle/recorders/http_foundation_session.php` (`scripts/record.sh
//! http-foundation-session`): 1,500 generated scripts.
//!
//! Each script builds a session on the array or file storage (attribute
//! bag, a flash bag or an auto-expiring one, storage keys from a pool, a
//! metadata bag with a threshold and a cookie lifetime, a usage reporter
//! that counts its calls), sometimes on tampered session data. It then
//! runs 4 to 20 operations, with the clock moving on between them, the
//! usage index set (as `SessionListener` sets it to `PHP_INT_MIN`), and
//! "reopen" starting a new storage on the same id.
//!
//! Compared: each result or exception, the usage index and the reporter's
//! count, and after each save the file's data. A generated id is `{id}` on
//! both sides.
//!
//! Upstream: none

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use php_value::{
    Key, PhpArray, Value, clock,
    constants::Registry,
    serialize::{Classes, unserialize},
    vectors,
};
use serde_json::Value as Json;
use symfony_http_foundation::{
    exception::Error,
    session::{
        Session,
        attribute::attribute_bag::AttributeBag,
        flash::{
            auto_expire_flash_bag::AutoExpireFlashBag, flash_bag::FlashBag,
            flash_bag_interface::FlashBagInterface,
        },
        flash_bag_aware_session_interface::FlashBagAwareSessionInterface,
        session_interface::SessionInterface,
        storage::{
            metadata_bag::MetadataBag, mock_array_session_storage::MockArraySessionStorage,
            mock_file_session_storage::MockFileSessionStorage,
            session_storage_interface::SessionStorageInterface,
        },
    },
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/session.json");

fn text(json: &Json) -> Result<&str, TestError> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn bytes(json: &Json) -> Result<Vec<u8>, TestError> {
    vectors::base64(text(json)?)
}

fn string(json: &Json) -> Result<String, TestError> {
    Ok(String::from_utf8(bytes(json)?)?)
}

fn items(json: &Json) -> Result<&Vec<Json>, TestError> {
    json.as_array().ok_or_else(|| "expected a list".into())
}

/// The recorder's `$value($spec)`.
fn value(spec: &Json) -> Result<Value, TestError> {
    Ok(match text(&spec[0])? {
        "s" => Value::String(bytes(&spec[1])?.into()),
        "i" => Value::Int(spec[1].as_i64().ok_or("int")?),
        "b" => Value::Bool(spec[1].as_bool().ok_or("bool")?),
        "a" => Value::Array(PhpArray::list(
            items(&spec[1])?
                .iter()
                .map(value)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        _ => Value::Null,
    })
}

/// `array_combine()` of base64 keys and value specs.
fn combine(pairs: &Json) -> Result<PhpArray, TestError> {
    let mut array = PhpArray::new();
    for pair in items(pairs)? {
        array.insert(Key::from_bytes(&bytes(&pair[0])?), value(&pair[1])?);
    }
    Ok(array)
}

/// `{id}` for a generated session id, as the recorder writes it.
fn normalize(value: Value) -> Value {
    match value {
        Value::String(s)
            if s.as_bytes().len() == 32
                && s.as_bytes()
                    .iter()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b)) =>
        {
            Value::string("{id}")
        }
        Value::Array(array) => {
            let mut out = PhpArray::new();
            for (k, v) in array.iter() {
                out.insert(k.clone(), normalize(v.clone()));
            }
            Value::Array(out)
        }
        other => other,
    }
}

struct Config {
    file: bool,
    dir: String,
    attribute_key: String,
    auto: bool,
    flash_key: String,
    threshold: i64,
    cookie_lifetime: Option<i64>,
}

/// The recorder's `$make()`.
type Made = (
    Arc<dyn SessionStorageInterface>,
    Session,
    Option<Arc<MockArraySessionStorage>>,
);

fn make(config: &Config, reported: &Arc<AtomicI64>) -> Result<Made, TestError> {
    let meta = Arc::new(MetadataBag::new(
        "_sf2_meta",
        config.threshold,
        config.cookie_lifetime,
    ));
    let (storage, array): (Arc<dyn SessionStorageInterface>, _) = if config.file {
        let storage = MockFileSessionStorage::new(
            Some(config.dir.as_bytes().to_vec()),
            "MOCKSESSID",
            Some(meta),
        )?;
        (Arc::new(storage), None)
    } else {
        let storage = Arc::new(MockArraySessionStorage::new("MOCKSESSID", Some(meta)));
        (storage.clone(), Some(storage))
    };
    let flashes: Arc<dyn FlashBagInterface> = if config.auto {
        Arc::new(AutoExpireFlashBag::new(config.flash_key.clone()))
    } else {
        Arc::new(FlashBag::new(config.flash_key.clone()))
    };
    let counter = Arc::clone(reported);
    let session = Session::new(
        Arc::clone(&storage),
        Some(Arc::new(AttributeBag::new(config.attribute_key.clone()))),
        Some(flashes),
        Some(Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        })),
    )?;
    Ok((storage, session, array))
}

/// The session data `setSessionData()` is given.
fn session_data(data: &Json) -> Result<PhpArray, TestError> {
    let mut array = PhpArray::new();
    for pair in items(data)? {
        let entry = &pair[1];
        let v = if entry[0] == "meta" {
            let mut meta = PhpArray::new();
            for (name, i) in [("c", 1), ("u", 2), ("l", 3)] {
                if let Some(n) = entry[i].as_i64() {
                    meta.insert(name, Value::Int(n));
                }
            }
            Value::Array(meta)
        } else {
            value(entry)?
        };
        array.insert(Key::from_bytes(text(&pair[0])?.as_bytes()), v);
    }
    Ok(array)
}

#[derive(Debug)]
enum Outcome {
    Value(Value),
    Saved(Option<Value>),
}

fn null(result: Result<(), Error>) -> Result<Value, Error> {
    result.map(|()| Value::Null)
}

#[allow(clippy::too_many_lines, reason = "one operation per recorded kind")]
fn run(
    op: &Json,
    config: &Config,
    storage: &mut Arc<dyn SessionStorageInterface>,
    session: &mut Session,
    reported: &Arc<AtomicI64>,
) -> Result<Result<Outcome, Error>, TestError> {
    let arg = |i: usize| string(&op[i]);
    let flash = |session: &Session| session.get_flash_bag();
    let got: Result<Value, Error> = match text(&op[0])? {
        "start" => session.start().map(Value::Bool),
        "set" => null(session.set(&arg(1)?, value(&op[2])?)),
        "get" => session.get(&arg(1)?, value(&op[2])?),
        "has" => session.has(&arg(1)?).map(Value::Bool),
        "remove" => session.remove(&arg(1)?),
        "all" => session.all().map(Value::Array),
        "replace" => null(session.replace(&combine(&op[1])?)),
        "clear" => null(session.clear()),
        "invalidate" => session.invalidate(op[1].as_i64()).map(Value::Bool),
        "migrate" => session
            .migrate(op[1].as_bool().ok_or("destroy")?, op[2].as_i64())
            .map(Value::Bool),
        "save" => {
            return Ok(session.save().map(|()| {
                if config.file {
                    let path = format!("{}/{}.mocksess", config.dir, storage.get_id());
                    Outcome::Saved(std::fs::read(&path).ok().and_then(|data| {
                        let mut classes = Classes::new();
                        classes.allow("stdClass");
                        unserialize(&data, &classes, &Registry::default()).map(normalize)
                    }))
                } else {
                    Outcome::Value(Value::Null)
                }
            }));
        }
        "id" => Ok(Value::string(session.get_id().as_str())),
        "set_id" => null(session.set_id(&arg(1)?)),
        "name" => Ok(Value::string(session.get_name().as_str())),
        "is_started" => Ok(Value::Bool(session.is_started())),
        "is_empty" => Ok(Value::Bool(session.is_empty())),
        "count" => session
            .count()
            .map(|n| Value::Int(i64::try_from(n).unwrap_or(i64::MAX))),
        "f_add" => {
            let (kind, message) = (arg(1)?, value(&op[2])?);
            flash(session).and_then(|f| null(f.add(&kind, message)))
        }
        "f_set" => {
            let (kind, messages) = (arg(1)?, value(&op[2])?);
            flash(session).and_then(|f| null(f.set(&kind, messages)))
        }
        "f_peek" => {
            let kind = arg(1)?;
            flash(session).and_then(|f| f.peek(&kind, PhpArray::new()).map(Value::Array))
        }
        "f_peek_all" => flash(session).and_then(|f| f.peek_all().map(Value::Array)),
        "f_get" => {
            let kind = arg(1)?;
            flash(session).and_then(|f| f.get(&kind, PhpArray::new()).map(Value::Array))
        }
        "f_all" => flash(session).and_then(|f| f.all().map(Value::Array)),
        "f_set_all" => {
            let messages = combine(&op[1])?;
            flash(session).and_then(|f| null(f.set_all(messages)))
        }
        "f_has" => {
            let kind = arg(1)?;
            flash(session).and_then(|f| f.has(&kind).map(Value::Bool))
        }
        "f_keys" => flash(session).and_then(|f| f.keys().map(|k| Value::Array(PhpArray::list(k)))),
        "m_created" => session.get_metadata_bag().get_created().map(Value::Int),
        "m_last_used" => session.get_metadata_bag().get_last_used().map(Value::Int),
        "m_lifetime" => session.get_metadata_bag().get_lifetime().map(Value::Int),
        "usage" => {
            // as SessionListener does, through &getUsageIndex()
            session
                .get_usage_index()
                .store(op[1].as_i64().ok_or("index")?, Ordering::Relaxed);
            Ok(Value::Null)
        }
        "reopen" => {
            let id = storage.get_id();
            let (new_storage, new_session, _) = make(config, reported)?;
            *storage = new_storage;
            *session = new_session;
            null(storage.set_id(&id))
        }
        other => return Err(format!("unknown op {other}").into()),
    };
    Ok(got.map(|v| Outcome::Value(normalize(v))))
}

fn same(recorded: &Json, got: &Result<Outcome, Error>) -> Result<bool, TestError> {
    Ok(match got {
        Err(e) => {
            recorded[0] == "throws"
                && recorded[1] == e.class().name()
                && bytes(&recorded[2])? == e.message().as_bytes()
        }
        Ok(Outcome::Saved(read)) => {
            recorded[0] == "saved"
                && match (&recorded[1], read) {
                    (Json::Null, None) => true,
                    (Json::Null, Some(_)) | (_, None) => false,
                    (expected, Some(value)) => vectors::decode(expected)?.identical(value),
                }
        }
        Ok(Outcome::Value(value)) => {
            recorded[0] != "throws"
                && recorded[0] != "saved"
                && vectors::decode(recorded)?.identical(value)
        }
    })
}

#[test]
fn sessions_answer_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let scripts = items(&data["scripts"])?;
    assert_eq!(1500, scripts.len());
    let dir = std::env::temp_dir().join(format!(
        "symfony-rs-session-recorded-{}",
        std::process::id()
    ));
    let dir_text = dir.to_string_lossy().replace('\\', "/");
    let mut failures = Vec::new();
    let mut answers = 0;

    for (n, script) in scripts.iter().enumerate() {
        let _ = std::fs::remove_dir_all(&dir);
        let _clock = clock::mock(1_700_000_000);
        let config = Config {
            file: script["kind"] == "file",
            dir: dir_text.clone(),
            attribute_key: text(&script["attributeKey"])?.to_owned(),
            auto: script["flashKind"] == "auto",
            flash_key: text(&script["flashKey"])?.to_owned(),
            threshold: script["threshold"].as_i64().ok_or("threshold")?,
            cookie_lifetime: script["cookieLifetime"].as_i64(),
        };
        let reported = Arc::new(AtomicI64::new(0));
        let (mut storage, mut session, array_storage) = make(&config, &reported)?;
        if !script["data"].is_null()
            && let Some(array_storage) = array_storage
        {
            array_storage.set_session_data(&session_data(&script["data"])?);
        }

        for (i, (op, recorded)) in items(&script["ops"])?
            .iter()
            .zip(items(&script["results"])?)
            .enumerate()
        {
            let seconds = items(op)?.last().and_then(Json::as_i64).ok_or("seconds")?;
            clock::sleep(seconds);
            answers += 1;
            let got = run(op, &config, &mut storage, &mut session, &reported)?;
            let usage = session.get_usage_index().load(Ordering::Relaxed);
            let count = reported.load(Ordering::Relaxed);
            if !same(&recorded[0], &got)?
                || recorded[1].as_i64() != Some(usage)
                || recorded[2].as_i64() != Some(count)
            {
                failures.push(format!(
                    "script {n} op {i} {op}: expected {} usage {} reported {}, got {got:?} usage {usage} reported {count}",
                    recorded[0], recorded[1], recorded[2]
                ));
                break;
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    let total = failures.len();
    failures.truncate(15);
    assert!(
        failures.is_empty(),
        "{total} scripts differ (of {answers} answers compared):\n{}",
        failures.join("\n")
    );
    eprintln!("{answers} answers");
    Ok(())
}
