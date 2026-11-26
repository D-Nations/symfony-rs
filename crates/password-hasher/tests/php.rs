//! The port against PHP 8.4.27's answers in `tests/data/php.json`
//! (`oracle/recorders/password_hasher.php`, `scripts/record.sh
//! password-hasher`): the hashers' outputs and errors, the PHP functions
//! they call, hashes PHP made, Argon2 strings, `needsRehash()` answers,
//! factory configurations, and PHP's verdict on hashes this port made.
//!
//! Every difference is listed in README.md ("Differences") and pinned
//! here as such: the hash algorithms not ported and the DES `crypt()`
//! formats.
//!
//! Upstream: none
#![allow(clippy::too_many_lines, clippy::unwrap_used)]

use std::sync::{Arc, OnceLock};

use php_value::{PhpArray, Value};
use serde_json::{Value as Json, json};
use symfony_password_hasher::{
    Error, HasherConfig, HasherSubject, MessageDigestPasswordHasher, MigratingPasswordHasher,
    NativePasswordHasher, PasswordHasherFactory, PasswordHasherFactoryInterface,
    PasswordHasherInterface, Pbkdf2PasswordHasher, PlaintextPasswordHasher, SodiumPasswordHasher,
    php::{
        argon2::{self, Variant},
        crypt::{Crypt, crypt},
        hash::{HashAlgorithm, bin2hex},
        password::password_verify,
    },
};

fn data() -> &'static Json {
    static DATA: OnceLock<Json> = OnceLock::new();
    DATA.get_or_init(|| {
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/php.json"))
                .unwrap();
        serde_json::from_str(&text).unwrap()
    })
}

fn cases(name: &str) -> &'static [Json] {
    data()[name].as_array().unwrap()
}

fn s(json: &Json) -> &str {
    json.as_str().unwrap()
}

fn opt(json: &Json) -> Option<&str> {
    json.as_str()
}

/// A recorded outcome as JSON: `{"value": ...}` or `{"throws": class,
/// "message": text}`.
fn outcome<T: Into<Json>>(result: Result<T, Error>) -> Json {
    match result {
        Ok(value) => json!({ "value": value.into() }),
        Err(e) => json!({ "throws": e.class().name(), "message": e.message() }),
    }
}

/// PHP's `\TypeError` messages end with ", called in <file> on line <n>",
/// which the port leaves out.
fn without_location(mut outcome: Json) -> Json {
    if let Some(message) = outcome.get("message").and_then(Json::as_str)
        && let Some(at) = message.find(", called in ")
    {
        outcome["message"] = Json::from(&message[..at]);
    }
    outcome
}

const NOT_PORTED: &str = "README.md, Differences";

#[test]
fn hash_algos() {
    let algos: Vec<&str> = cases("hash_algos").iter().map(s).collect();
    let ported: Vec<&str> = algos
        .iter()
        .copied()
        .filter(|a| HashAlgorithm::from_name(a).is_some())
        .collect();
    assert_eq!(
        vec![
            "md5",
            "sha1",
            "sha224",
            "sha256",
            "sha384",
            "sha512/224",
            "sha512/256",
            "sha512"
        ],
        ported
    );
    assert_eq!(
        60,
        algos.len(),
        "{NOT_PORTED}: 52 algorithms are not ported"
    );
}

#[test]
fn message_digest() {
    for case in cases("digest") {
        let hasher = MessageDigestPasswordHasher::new(
            s(&case["algorithm"]),
            case["base64"].as_bool().unwrap(),
            case["iterations"].as_i64().unwrap(),
        );
        let actual = outcome(hasher.hash(s(&case["password"]), opt(&case["salt"])));
        assert_eq!(case["hash"], actual, "{case}");
    }
    for case in cases("digest_unsupported") {
        let hasher = MessageDigestPasswordHasher::new(s(&case["algorithm"]), true, 1);
        let hash = outcome(hasher.hash("password", Some("salt")));
        let algorithm = s(&case["algorithm"]);
        if ["foobar", "SHA256"].contains(&algorithm) {
            assert_eq!(case["hash"], hash);
        } else {
            // PHP computes these; the port does not (README.md).
            assert!(case["hash"].get("value").is_some());
            assert_eq!(
                json!({
                    "throws": "Symfony\\Component\\PasswordHasher\\Exception\\LogicException",
                    "message": format!("The algorithm \"{algorithm}\" is not supported."),
                }),
                hash
            );
        }
        assert_eq!(
            case["verify"],
            outcome(hasher.verify("", "password", Some("salt")))
        );
    }
    let hasher = MessageDigestPasswordHasher::new("sha256", false, 1);
    for case in cases("digest_verify") {
        let actual =
            outcome(hasher.verify(s(&case["hash"]), s(&case["password"]), opt(&case["salt"])));
        assert_eq!(case["verify"], actual, "{case}");
    }
}

#[test]
fn pbkdf2() {
    for case in cases("pbkdf2") {
        let hasher = Pbkdf2PasswordHasher::new(
            s(&case["algorithm"]),
            case["base64"].as_bool().unwrap(),
            case["iterations"].as_i64().unwrap(),
            case["length"].as_i64().unwrap(),
        )
        .unwrap();
        let actual = outcome(hasher.hash(s(&case["password"]), opt(&case["salt"])));
        assert_eq!(case["hash"], actual, "{case}");
    }
    for case in cases("pbkdf2_construct") {
        let args = case["arguments"].as_array().unwrap();
        let algorithm = s(&args[0]);
        let actual = outcome(
            Pbkdf2PasswordHasher::new(
                algorithm,
                args[1].as_bool().unwrap(),
                args[2].as_i64().unwrap(),
                args[3].as_i64().unwrap(),
            )
            .and_then(|h| h.verify("", "password", Some("salt"))),
        );
        if algorithm == "crc32" {
            // PHP's hash_pbkdf2() refuses crc32 with a \ValueError from
            // the constructor; the port does not know crc32 (README.md).
            assert_eq!("ValueError", case["construct"]["throws"]);
            assert_eq!(json!({ "value": false }), actual);
        } else {
            assert_eq!(case["construct"], actual, "{case}");
        }
    }
    let hasher = Pbkdf2PasswordHasher::new("sha256", true, 1, 40).unwrap();
    for case in cases("pbkdf2_verify") {
        let actual =
            outcome(hasher.verify(s(&case["hash"]), s(&case["password"]), opt(&case["salt"])));
        assert_eq!(case["verify"], actual, "{case}");
    }
}

#[test]
fn plaintext() {
    for case in cases("plaintext") {
        let hasher = PlaintextPasswordHasher::new(case["ignore_case"].as_bool().unwrap());
        let (password, salt) = (s(&case["password"]), opt(&case["salt"]));
        assert_eq!(
            case["hashed"],
            outcome(hasher.hash(password, salt)),
            "{case}"
        );
        let verify = outcome(hasher.verify(s(&case["hash"]), password, salt));
        assert_eq!(case["verify"], verify, "{case}");
    }
}

#[test]
fn crypt_formats() {
    let mut unsupported = 0;
    for case in cases("crypt") {
        let (password, setting) = (s(&case["password"]), s(&case["setting"]));
        let expected = s(&case["crypt"]);
        match crypt(password.as_bytes(), setting.as_bytes()) {
            Crypt::Hash(hash) => assert_eq!(expected.as_bytes(), hash, "{case}"),
            Crypt::Failure(failure) => assert_eq!(expected, failure, "{case}"),
            Crypt::Unsupported => {
                // DES, and $2x$ with a byte above 0x7F (README.md).
                let des = !setting.starts_with('$');
                let two_x = setting.starts_with("$2x$") && !password.is_ascii();
                assert!(des || two_x, "{case}");
                assert!(!expected.starts_with('*'), "{case}");
                unsupported += 1;
            }
        }
    }
    assert_eq!(16 + 2, unsupported);

    for case in cases("password_verify") {
        let (password, hash) = (s(&case["password"]), s(&case["hash"]));
        let expected = case["verify"].as_bool().unwrap();
        let actual = password_verify(password.as_bytes(), hash);
        if hash.starts_with('$') {
            assert_eq!(expected, actual, "{case}");
        } else {
            // DES (README.md): never verified here.
            assert!(!actual, "{case}");
        }
    }
}

#[test]
fn argon2_outputs() {
    for case in cases("argon2_raw") {
        let variant = match s(&case["algorithm"]) {
            "argon2id" => Variant::Argon2id,
            _ => Variant::Argon2i,
        };
        let mem = case["mem"].as_u64().unwrap();
        let salt: Vec<u8> = (0..16)
            .map(|i| u8::from_str_radix(&s(&case["salt"])[i * 2..i * 2 + 2], 16).unwrap())
            .collect();
        let out = argon2::compute(
            variant,
            u32::try_from(mem / 1024).unwrap(),
            u32::try_from(case["ops"].as_u64().unwrap()).unwrap(),
            1,
            s(&case["password"]).as_bytes(),
            &salt,
            32,
        )
        .unwrap();
        assert_eq!(s(&case["out"]), bin2hex(&out), "{case}");
    }
}

#[test]
fn hashes_made_by_php() {
    let native = NativePasswordHasher::new(None, None, None, None).unwrap();
    let sodium = SodiumPasswordHasher::new(Some(3), Some(10240)).unwrap();
    for case in cases("made") {
        let (hash, password) = (s(&case["hash"]), s(&case["password"]));
        assert_eq!(
            case["native_verify"],
            json!(native.verify(hash, password, None).unwrap()),
            "{case}"
        );
        assert_eq!(
            case["sodium_verify"],
            json!(sodium.verify(hash, password, None).unwrap()),
            "{case}"
        );
        let other = format!("{password}x");
        assert_eq!(
            case["native_verify_other"],
            json!(native.verify(hash, &other, None).unwrap()),
            "{case}"
        );
    }
}

#[test]
fn argon2_strings() {
    for case in cases("argon2_strings") {
        let hash = s(&case["hash"]);
        assert_eq!(
            case["sodium_verify"],
            json!({ "value": argon2::sodium_verify(hash, b"password") }),
            "{case}"
        );
        assert_eq!(
            case["sodium_needs_rehash_3_10240"],
            json!({ "value": argon2::sodium_needs_rehash(hash, 3, 10240) }),
            "{case}"
        );
        for (name, algorithm) in [("argon2id_3_10", "argon2id"), ("argon2i_3_10", "argon2i")] {
            let hasher =
                NativePasswordHasher::new(Some(3), Some(10240), Some(13), Some(algorithm)).unwrap();
            assert_eq!(
                case["native_needs_rehash"][name],
                json!(hasher.needs_rehash(hash)),
                "{case}"
            );
        }
    }
}

#[test]
fn needs_rehash() {
    let natives: Vec<(&str, NativePasswordHasher)> = vec![
        ("native(4, 11000, 4)", (Some(4), Some(11000), Some(4), None)),
        ("native(5, 11000, 5)", (Some(5), Some(11000), Some(5), None)),
        ("native(null, null, 13)", (None, None, Some(13), None)),
        (
            "native(3, 10240, 4, argon2id)",
            (Some(3), Some(10240), Some(4), Some("argon2id")),
        ),
        (
            "native(3, 10240, 4, argon2i)",
            (Some(3), Some(10240), Some(4), Some("argon2i")),
        ),
        (
            "native(4, 11264, 4, argon2id)",
            (Some(4), Some(11264), Some(4), Some("argon2id")),
        ),
        (
            "native(4, 11000, 4, 3)",
            (Some(4), Some(11000), Some(4), Some("3")),
        ),
        (
            "native(4, 11000, 4, foo)",
            (Some(4), Some(11000), Some(4), Some("foo")),
        ),
        (
            "native(null, null, null, 2y)",
            (None, None, None, Some("2y")),
        ),
    ]
    .into_iter()
    .map(|(name, (o, m, c, a))| (name, NativePasswordHasher::new(o, m, c, a).unwrap()))
    .collect();
    let sodiums: Vec<(String, SodiumPasswordHasher)> = [
        (3, 10240),
        (4, 11000),
        (4, 11264),
        (4, 10240),
        (4, 65536 * 1024),
    ]
    .into_iter()
    .map(|(o, m)| {
        (
            format!("sodium({o}, {m})"),
            SodiumPasswordHasher::new(Some(o), Some(m)).unwrap(),
        )
    })
    .collect();
    for case in cases("needs_rehash") {
        let hash = s(&case["hash"]);
        for (name, hasher) in &natives {
            assert_eq!(
                case["native"][name],
                json!(hasher.needs_rehash(hash)),
                "{name} {hash}"
            );
        }
        for (name, hasher) in &sodiums {
            assert_eq!(
                case["sodium"][name],
                json!(hasher.needs_rehash(hash)),
                "{name} {hash}"
            );
        }
    }
}

#[test]
fn constructors() {
    for case in cases("native_construct") {
        let args = case["arguments"].as_array().unwrap();
        let actual = outcome(
            NativePasswordHasher::new(
                args[0].as_i64(),
                args[1].as_i64(),
                args[2].as_i64(),
                opt(&args[3]),
            )
            .map(|h| {
                let prefix = outcome(h.hash("x", None).map(|hash| hash[..7].to_owned()));
                json!({
                    "algorithm": h.algorithm().name(),
                    "options": {
                        "cost": h.options().cost,
                        "time_cost": h.options().time_cost,
                        "memory_cost": h.options().memory_cost,
                        "threads": h.options().threads,
                    },
                    "hash_prefix": prefix,
                })
            }),
        );
        assert_eq!(case["outcome"], actual, "{case}");
    }
    for case in cases("sodium_construct") {
        let args = case["arguments"].as_array().unwrap();
        let actual = outcome(
            SodiumPasswordHasher::new(args[0].as_i64(), args[1].as_i64())
                .map(|h| json!({ "opsLimit": h.ops_limit(), "memLimit": h.mem_limit() })),
        );
        assert_eq!(case["outcome"], actual, "{case}");
    }
    for case in cases("too_long") {
        let password = "a".repeat(usize::try_from(case["length"].as_u64().unwrap()).unwrap());
        let native = NativePasswordHasher::new(None, None, Some(4), None).unwrap();
        let sodium = SodiumPasswordHasher::new(Some(3), Some(10240)).unwrap();
        assert_eq!(
            case["native_hash"],
            outcome(native.hash(&password, None).map(|h| h.len()))
        );
        assert_eq!(
            case["sodium_hash"],
            outcome(sodium.hash(&password, None).map(|h| h.len()))
        );
        let digest = MessageDigestPasswordHasher::new("sha256", false, 1);
        assert_eq!(case["digest_hash"], outcome(digest.hash(&password, None)));
        let pbkdf2 = Pbkdf2PasswordHasher::new("sha256", false, 1, 8).unwrap();
        assert_eq!(case["pbkdf2_hash"], outcome(pbkdf2.hash(&password, None)));
        let plaintext = PlaintextPasswordHasher::default();
        assert_eq!(
            case["plaintext_hash"],
            outcome(plaintext.hash(&password, None).map(|h| h.len()))
        );
    }
}

/// A hasher's class and settings, as the recorder's `describe()` reads
/// them by reflection.
fn describe(hasher: &dyn PasswordHasherInterface) -> Json {
    if let Some(h) = hasher.downcast_ref::<NativePasswordHasher>() {
        let o = h.options();
        return json!({
            "class": "NativePasswordHasher",
            "algorithm": h.algorithm().name(),
            "options": { "cost": o.cost, "time_cost": o.time_cost, "memory_cost": o.memory_cost, "threads": o.threads },
        });
    }
    if let Some(h) = hasher.downcast_ref::<SodiumPasswordHasher>() {
        return json!({ "class": "SodiumPasswordHasher", "opsLimit": h.ops_limit(), "memLimit": h.mem_limit() });
    }
    if let Some(h) = hasher.downcast_ref::<MessageDigestPasswordHasher>() {
        return json!({
            "class": "MessageDigestPasswordHasher",
            "algorithm": h.algorithm(),
            "encodeHashAsBase64": h.encode_hash_as_base64(),
            "iterations": h.iterations(),
        });
    }
    if let Some(h) = hasher.downcast_ref::<Pbkdf2PasswordHasher>() {
        return json!({
            "class": "Pbkdf2PasswordHasher",
            "algorithm": h.algorithm(),
            "encodeHashAsBase64": h.encode_hash_as_base64(),
            "iterations": h.iterations(),
            "length": h.length(),
        });
    }
    if let Some(h) = hasher.downcast_ref::<PlaintextPasswordHasher>() {
        return json!({ "class": "PlaintextPasswordHasher", "ignorePasswordCase": h.ignore_password_case() });
    }
    if let Some(h) = hasher.downcast_ref::<MigratingPasswordHasher>() {
        let extras: Vec<Json> = h
            .extra_hashers()
            .iter()
            .map(|e| describe(e.as_ref()))
            .collect();
        return json!({
            "class": "MigratingPasswordHasher",
            "bestHasher": describe(h.best_hasher().as_ref()),
            "extraHashers": extras,
        });
    }
    panic!("unknown hasher {}", hasher.class_name());
}

/// A JSON value as PHP's `json_decode($json, true)` gives it.
fn php(json: &Json) -> Value {
    match json {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => n
            .as_i64()
            .map_or_else(|| Value::Float(n.as_f64().unwrap()), Value::Int),
        Json::String(s) => Value::string(s.as_str()),
        Json::Array(values) => Value::Array(PhpArray::list(values.iter().map(php))),
        Json::Object(map) => {
            let mut array = PhpArray::new();
            for (key, value) in map {
                array.insert(key.as_str(), php(value));
            }
            Value::Array(array)
        }
    }
}

fn factory(config: &Json) -> PasswordHasherFactory {
    let digest: Arc<dyn PasswordHasherInterface> =
        Arc::new(MessageDigestPasswordHasher::new("sha256", true, 5000));
    let mut named = PhpArray::new();
    named.insert("algorithm", Value::string("pbkdf2"));
    named.insert("iterations", Value::Int(9));
    let user = if config.as_str() == Some("digest_hasher") {
        HasherConfig::Hasher(digest.clone())
    } else {
        let Value::Array(mut options) = php(config) else {
            panic!("{config}");
        };
        let instance = options
            .remove(&php_value::Key::from_bytes(b"instance"))
            .map(|_| digest.clone());
        HasherConfig::Options { instance, options }
    };
    PasswordHasherFactory::new([
        ("digest_hasher", HasherConfig::Hasher(digest)),
        ("named_config", HasherConfig::options(named)),
        ("App\\User", user),
    ])
}

#[test]
fn factory_configurations() {
    for case in cases("factory") {
        let factory = factory(&case["config"]);
        for key in ["hasher", "again"] {
            let actual = outcome(
                factory
                    .get_password_hasher(HasherSubject::name("App\\User"))
                    .map(|h| describe(h.as_ref())),
            );
            assert_eq!(without_location(case[key].clone()), actual, "{case}");
        }
    }
    for case in cases("factory_subjects") {
        let factory = PasswordHasherFactory::new([
            (
                "digest_hasher",
                HasherConfig::Hasher(Arc::new(MessageDigestPasswordHasher::new(
                    "sha256", true, 5000,
                ))),
            ),
            (
                "App\\User",
                HasherConfig::Hasher(Arc::new(PlaintextPasswordHasher::default())),
            ),
        ]);
        let actual = outcome(
            factory
                .get_password_hasher(HasherSubject::name(s(&case["subject"])))
                .map(|h| describe(h.as_ref())),
        );
        assert_eq!(case["hasher"], actual, "{case}");
    }

    let mut auto = PhpArray::new();
    auto.insert("algorithm", Value::string("auto"));
    auto.insert("time_cost", Value::Int(3));
    auto.insert("memory_cost", Value::Int(10));
    auto.insert("cost", Value::Int(4));
    let factory = PasswordHasherFactory::new([("u", HasherConfig::options(auto))]);
    let hasher = factory
        .get_password_hasher(HasherSubject::name("u"))
        .unwrap();
    for case in cases("auto_verify") {
        let hash = s(&case["hash"]);
        assert_eq!(
            case["verify"],
            json!(hasher.verify(hash, "pw", None).unwrap()),
            "{case}"
        );
        assert_eq!(
            case["needs_rehash"],
            json!(hasher.needs_rehash(hash)),
            "{case}"
        );
    }
}

const RUST_HASHES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/rust_hashes.json");

/// Writes hashes made by this port for the recorder to verify with PHP:
/// `cargo test -p symfony-password-hasher --test php -- --ignored
/// write_rust_hashes`, then `scripts/record.sh password-hasher`.
#[test]
#[ignore = "writes tests/data/rust_hashes.json"]
fn write_rust_hashes() {
    let mut out = Vec::new();
    let hashers: Vec<(&str, Box<dyn PasswordHasherInterface>)> = vec![
        (
            "NativePasswordHasher(cost 4)",
            Box::new(NativePasswordHasher::new(None, None, Some(4), None).unwrap()),
        ),
        (
            "NativePasswordHasher(3, 10240, 4, argon2i)",
            Box::new(
                NativePasswordHasher::new(Some(3), Some(10240), Some(4), Some("argon2i")).unwrap(),
            ),
        ),
        (
            "NativePasswordHasher(3, 10240, 4, argon2id)",
            Box::new(
                NativePasswordHasher::new(Some(3), Some(10240), Some(4), Some("argon2id")).unwrap(),
            ),
        ),
        (
            "SodiumPasswordHasher(3, 10240)",
            Box::new(SodiumPasswordHasher::new(Some(3), Some(10240)).unwrap()),
        ),
        (
            "SodiumPasswordHasher()",
            Box::new(SodiumPasswordHasher::new(None, None).unwrap()),
        ),
    ];
    for (by, hasher) in &hashers {
        for password in [
            "password",
            "é",
            "a\0b",
            &"a".repeat(72),
            &"x".repeat(100),
            "Ünïcödé ✓",
        ] {
            out.push(json!({ "by": by, "password": password, "hash": hasher.hash(password, None).unwrap() }));
        }
    }
    let text = serde_json::to_string_pretty(&out).unwrap();
    std::fs::write(RUST_HASHES, text + "\n").unwrap();
}

#[test]
fn hashes_made_by_rust_verified_by_php() {
    let Some(recorded) = data().get("rust_hashes").and_then(Json::as_array) else {
        panic!("tests/data/php.json has no rust_hashes: run write_rust_hashes, then record");
    };
    assert!(!recorded.is_empty());
    let native = NativePasswordHasher::new(None, None, None, None).unwrap();
    for case in recorded {
        assert_eq!(json!(true), case["php_native_verify"], "{case}");
        assert_eq!(json!(true), case["php_sodium_verify"], "{case}");
        assert_eq!(json!(false), case["php_native_verify_other"], "{case}");
        assert!(
            native
                .verify(s(&case["hash"]), s(&case["password"]), None)
                .unwrap()
        );
        let algo = case["php_info"]["algoName"].as_str().unwrap();
        assert!(["bcrypt", "argon2i", "argon2id"].contains(&algo), "{case}");
    }
}
