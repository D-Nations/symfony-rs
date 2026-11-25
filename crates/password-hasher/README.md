# Symfony PasswordHasher

Private workspace crate: a port of `symfony/password-hasher` v8.1.0
(`6934d16b`), as installed by symfony/demo. MIT; Symfony source and tests
copyright Fabien Potencier. No PHP source code is used: the PHP functions
in `src/php/` (`password_hash()`, `password_verify()`, `crypt()`,
`sodium_crypto_pwhash_str*()`, `hash()`, `hash_pbkdf2()`) are written from
their documentation and from PHP's recorded answers. Unofficial, not
affiliated with or endorsed by Symfony or PHP.

```rust
use symfony_password_hasher::{NativePasswordHasher, PasswordHasherInterface};

let hasher = NativePasswordHasher::new(None, None, Some(4), None).unwrap();
let hash = hasher.hash("s3cret", None).unwrap();
assert!(hash.starts_with("$2y$04$"));
assert!(hasher.verify(&hash, "s3cret", None).unwrap());
assert!(!hasher.needs_rehash(&hash));
```

| Contract | Source | Evidence |
|---|---|---|
| bcrypt hashes in PHP's `$2y$` format; passwords longer than 72 bytes or with a NUL byte pre-hashed with SHA-512; verification of `$2a$`, `$2b$`, `$2y$` and (ASCII passwords) `$2x$` hashes; `needsRehash()` on algorithm and cost | `NativePasswordHasher.php` | NativePasswordHasherTest (15); PHP verifies the Rust hashes and Rust verifies PHP's; `crypt()` vectors |
| Argon2i and Argon2id through `password_hash()` (`$argon2i$v=19$m=..,t=..,p=1$...`), verified as libsodium does; `password_needs_rehash()`'s reading of the options | `NativePasswordHasher.php` | Same Argon2 outputs as `sodium_crypto_pwhash()` for 32 salts; 28 odd Argon2 strings; 25 hashes x 14 hasher settings for `needsRehash()` |
| libsodium's `crypto_pwhash_str()` format (Argon2id, one lane), its strict reading for verification and `needs_rehash`; bcrypt and SHA-crypt hashes accepted for migration | `SodiumPasswordHasher.php` | SodiumPasswordHasherTest (9); PHP verifies the Rust hashes |
| `password_verify()` of `$5$` and `$6$` (SHA-crypt, `rounds=`) and `$1$` (MD5-crypt) hashes; the password ends at a NUL byte, as in PHP | `NativePasswordHasher.php`, `SodiumPasswordHasher.php` | 368 `crypt()` vectors, 90 `password_verify()` vectors |
| Message digest hashes (`md5`, `sha1`, `sha224`, `sha256`, `sha384`, `sha512/224`, `sha512/256`, `sha512`), stretched, base64 or hex; the `{salt}` merge and `Cannot use { or } in salt.` | `MessageDigestPasswordHasher.php` | MessageDigestPasswordHasherTest (6); 1,680 hashes and 10 verifications |
| PBKDF2 with HMAC of the same algorithms, any key length; `hash_pbkdf2()`'s `\ValueError` for iterations below 1 or a negative length, from the constructor | `Pbkdf2PasswordHasher.php` | Pbkdf2PasswordHasherTest (5); 2,304 hashes |
| Plain text, with ASCII case folding | `PlaintextPasswordHasher.php` | PlaintextPasswordHasherTest (5) |
| The 4096-byte limit (`InvalidPasswordException`, `Invalid password.`) | `CheckPasswordLengthTrait.php`, `Exception/*.php` | Tests of each hasher; oracle vectors |
| Verification through a chain of hashers | `MigratingPasswordHasher.php` | MigratingPasswordHasherTest (2) |
| Hashers by user class (with parent classes and interfaces), by hasher name (`PasswordHasherAwareInterface`) or by string; `algorithm` (`auto`, `bcrypt`, `native`, `sodium`, `argon2i`, `argon2id`, `pbkdf2`, `plaintext`, digest names), `migrate_from`, `instance`, `class` and `arguments` (positional and named, PHP's scalar coercion); the default migration chains; upstream's messages | `PasswordHasherFactory.php` | PasswordHasherFactoryTest (16); 36 configurations and 3 subjects recorded |
| Hash, verify and `needsRehash()` for a user, with the salt of a legacy user | `UserPasswordHasher.php` | UserPasswordHasherTest (5) |

Vectors: `scripts/record.sh password-hasher` runs
`oracle/recorders/password_hasher.php` on PHP 8.4.27 (the oracle image's
base, `php:8.4.27-cli-bookworm`, which has sodium 1.0.18 and libargon2)
and writes `tests/data/php.json`; `tests/php.rs` checks them. The
digest, PBKDF2, `crypt()` and raw Argon2 vectors re-record byte for byte;
the hashes made by `password_hash()` and `sodium_crypto_pwhash_str()` have
random salts and change on each recording. To let PHP verify hashes made
here, run `cargo test -p symfony-password-hasher --test php -- --ignored
write_rust_hashes` (it writes `tests/data/rust_hashes.json`) and record
again. The current file has 30 Rust hashes (bcrypt, Argon2i, Argon2id,
sodium), and PHP verifies all of them.

## Differences

- `MessageDigestPasswordHasher` and `Pbkdf2PasswordHasher` compute the 8
  MD5, SHA-1 and SHA-2 algorithms only. For the other 52 names of PHP's
  `hash_algos()` (`md4`, `sha3-256`, `whirlpool`, `crc32`, ...) the
  hashers report `The algorithm "x" is not supported.`, as PHP does for
  an unknown name. `tests/php.rs` pins this.
- `password_verify()` does not compute the DES formats of `crypt()`
  (standard `ab...` and extended `_...`) and gives `false` for them. It
  also gives `false` for a `$2x$` hash (bcrypt with the old
  sign-extension bug) and a password with a byte above 0x7F.
- Passwords, salts and hashes are `&str` (UTF-8). PHP strings can hold any
  bytes.
- `PasswordHasherFactory`: `new $class(...)` knows the hasher classes of
  this package only; another class is `Class "x" not found`. A
  `\TypeError` message stops before PHP's `, called in <file> on line
  <n>`. A configuration that makes PHP recurse without end (for example
  `hash_algorithm: bcrypt`, or `auto` in `hash_algorithm`) gives an
  `\Error` here; PHP exhausts its stack.
- The environment is that of the oracle: libsodium 1.0.18 with Argon2id,
  and `PASSWORD_ARGON2I` and `PASSWORD_ARGON2ID`.
  `SodiumPasswordHasher::is_supported()` is always `true`, and the
  "not available" branches of PHP do not occur.

## Rust shape

- One trait, `PasswordHasherInterface`, has the methods of both PHP
  interfaces. Each method takes the salt; the self-salting hashers ignore
  it, as PHP ignores the extra argument. `LegacyPasswordHasherInterface`
  marks the salted hashers (`is_legacy()` answers `instanceof`).
  `verify()` returns a `Result`, because the salted hashers throw from it
  for a salt with `{` or `}`. `downcast_ref()` answers `instanceof` for a
  concrete class.
- Exceptions are one `Error` with its `Class` (`InvalidPasswordException`,
  `LogicException`, `\InvalidArgumentException`, `\RuntimeException`,
  `\ValueError`, ...); `is_logic()` and `is_runtime()` answer `instanceof`.
- Security Core's `PasswordAuthenticatedUserInterface` and
  `LegacyPasswordAuthenticatedUserInterface` are traits in `src/user.rs`
  until a security-core crate holds them. A user tells its class and its
  parents (`PhpClass`), and the factory compares class names
  case-insensitively, as PHP does.
- The factory's option arrays are `php-value` arrays; an `instance` is a
  hasher next to them (`HasherConfig`). The factory is `Send + Sync`,
  and it keeps each hasher after it is created.
- The hash crates are optimized also in development builds (workspace
  `Cargo.toml`), so tests with upstream's default costs (bcrypt 13,
  Argon2id 64 MiB) take seconds.

## Remaining work

- `Command/UserPasswordHashCommand.php` and its 24 tests stay `pending`:
  the command needs Console's `QuestionHelper` (hidden questions, choice,
  confirmation), which `symfony-console` does not have yet.
