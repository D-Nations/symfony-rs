# M2-A: Session, stage 1

November 26. Branch `m2/session`. Crate touched: `symfony-http-foundation`
(the new `session` module, and `php`: `Reference` and the helpers for
PHP's `[]`, `array` and `int` types).

New dependency for the crate: `getrandom`, already in the workspace; its
PLAN stack row now covers session ids. Oracle: a new recorder,
`http_foundation_session.php`.

## Scope

| PHP files (symfony/http-foundation v8.1.11) | Rust | Upstream tests |
|---|---|---|
| `Session/Session.php`, `SessionInterface.php`, `FlashBagAwareSessionInterface.php`, `SessionFactory.php`, `SessionFactoryInterface.php` | `crates/http-foundation/src/session/` | SessionTest 24 of 24 |
| `SessionBagInterface.php`, `SessionBagProxy.php` | `session/session_bag_interface.rs`, `session_bag_proxy.rs` | through SessionTest |
| `Attribute/AttributeBag.php`, `AttributeBagInterface.php` | `session/attribute/` | AttributeBagTest 13 of 13 |
| `Flash/FlashBag.php`, `AutoExpireFlashBag.php`, `FlashBagInterface.php` | `session/flash/` | FlashBagTest 12 of 12, AutoExpireFlashBagTest 13 of 13 |
| `Storage/MetadataBag.php`, `SessionStorageInterface.php`, `SessionStorageFactoryInterface.php` | `session/storage/` | MetadataBagTest 11 of 11 |
| `Storage/MockArraySessionStorage.php`, `MockFileSessionStorage.php`, `MockFileSessionStorageFactory.php` | `session/storage/` | MockArraySessionStorageTest 7 of 7, MockFileSessionStorageTest 9 of 9 |

The rest of Session is left for later stages. That is
`NativeSessionStorage`, `PhpBridgeSessionStorage`, the proxies, the save
handlers and `SessionUtils`, which all rest on PHP's session extension.

## Prior art and choice

The Rust session crates found were tower-sessions, axum-messages (flash
messages on top of tower-sessions) and rsession. Each has its own data
model: tower-sessions and axum-messages follow Django's sessions and
messages, and they store their data their own way. None has Symfony's bags,
metadata or serialized storage, so none was adopted, and the classes are
ported as they are.

**PHP references:** what matters most is how PHP shares data by
reference. A storage hands each bag its part of the session data
(`initialize(array &$array)`), and `SessionBagProxy` also keeps it in the
session's own list. They are all the same values. Here they are a shared
`php::Reference`, and a storage's data is a list of keys and references.

That list behaves as a PHP array of references does:

- a copy keeps the same references, as `MockFileSessionStorage::save()`
  relies on;
- `setSessionData()` and `clear()` give the storage new references, and
  bags keep the old ones until they are initialized again.

**Classes:** bags and storages are shared trait objects that change
through `&self`. PHP's overriding of `start()` by `MockFileSessionStorage`,
which the parent's `getBag()` and `regenerate()` call, becomes a `start`
passed to those methods.

## Evidence

- **Upstream tests:** 89 of 89 for these classes. No GitHub advisory
  covers these files.
- **Recorded vectors:** 1,500 scripts recorded from PHP 8.4.27
  (`scripts/record.sh http-foundation-session`, 17,937 answers), all
  matching. Each script builds a session on the array or file storage,
  with:
  - a plain or auto-expiring flash bag, and storage keys from a pool;
  - a metadata bag with an update threshold and a cookie lifetime;
  - a usage reporter that counts its calls;
  - sometimes tampered data: strings where bags expect arrays, metadata
    without a creation time.

  It then runs up to 20 operations, among them:
  - the session's, flash bag's and metadata bag's methods;
  - ids that are not file names;
  - the usage index set as `SessionListener` sets it;
  - a reopening on the same id, as the next request makes.

  The clock moves on between operations. Each result or exception is
  compared, with the usage index, the reporter's count and every saved
  file.
- **Found by the vectors:**
  - `SessionBagProxy::initialize()` checks its `array` parameter before
    counting the use, so tampered data throws the proxy's `TypeError`.
  - A new metadata array gets `u` before `c`: PHP's chained assignment
    `$meta[CREATED] = $meta[UPDATED] = ...` runs from the right, and the
    saved files show the order.
- **Mutations, each caught** (scripts that differ, of 1,500):
  - the proxy counting before the type check (114);
  - `c` before `u` (160);
  - the update threshold ignored (3);
  - a falsy flash entry counted as present (1);
  - the auto-expiring bag keeping `display` (583);
  - empty bags saved (150);
  - a lone metadata entry saved (40);
  - `getMetadataBag()` not counted as a use (935);
  - `isEmpty()` counting before the start (134);
  - the reporter called on a negative index (225);
  - `setId()` with the same id passed on (2);
  - bags given copies instead of references (264).

## Checks

See the PR for the output of the full gate on the final commit.

The symfony-demo-rs comparison was not run: the owner paused it
(November 23).

## Left open

- **No default storage:** `new Session()` without a storage would create
  a `NativeSessionStorage`, which is not ported, so a storage must be
  given.
- **`ini_get('session.cookie_lifetime')`** is `0`, PHP's default.
- **Unserialized objects:** only `stdClass` is created; other classes come
  back as `__PHP_Incomplete_Class`.
- **Next:** `NativeSessionStorage` and the save handlers (PHP's session
  extension), then `Request`'s session methods.
