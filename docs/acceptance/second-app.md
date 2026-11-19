# Second application: EasyAdmin demo

The M1-D second application (PLAN.md decision 10, a recommendation used
unless the owner changes it). It is the reference for ranking component
work beside the Symfony demo. Nothing here claims that any of it runs in
Rust yet.

| | |
|---|---|
| Repository | [EasyCorp/easyadmin-demo](https://github.com/EasyCorp/easyadmin-demo) |
| Pinned commit | `119c6df2921867eb59cae083f807b8bb3cdd694a` (2026-11-11), in `upstream.tsv` as `apps/easyadmin-demo` |
| Licence | MIT (`LICENSE.md`, "Copyright (c) 2015-present EasyCorp"). Every locked runtime package is MIT, except Twig (BSD-3-Clause) |
| Requires | PHP ≥ 8.4, pdo_sqlite, intl; `symfony/*` `^8.1` (`extra.symfony.require: 8.1.*`) |
| Oracle | `symfony-rs-oracle:8.4.27` already has PHP 8.4.27, intl and pdo_sqlite. Nothing was added to the image |

## Why this application

Candidates checked on 2026-11-19, by the `symfony/framework-bundle`
constraint on each default branch:

| Application | Licence | Symfony | Verdict |
|---|---|---|---|
| EasyCorp/easyadmin-demo | MIT | `^8.1`, PHP ≥ 8.4 | **Chosen**: the version line already pinned here, maintained, a different shape from the Symfony demo (admin CRUD, enums, ICU translations, CSV streaming) |
| sulu/skeleton | MIT | `^8.1` only on the 3.1 branch, with `sulu/sulu ~3.1.0-alpha1` | Alpha CMS; much larger than an acceptance app needs |
| Part-DB/Part-DB-server | AGPL-3.0 | `7.4.*` | Wrong version line |
| kimai/kimai | AGPL-3.0 | `^6.0` | Wrong version line |
| bolt/core | MIT | `^6.4` | Wrong version line |
| wallabag/wallabag | MIT | `^5.4.45` | Wrong version line |
| sulu/skeleton 3.0, Sylius-Standard 2.3 | MIT | `^7.4` / `^7.4 \|\| ^8.0` | Wrong version line (Sylius is also a full shop) |

## Version fit

The lock uses the 8.1 line, but newer patch releases than
`parity/packages.tsv` pins from symfony/demo. Of the 60 pinned packages
it installs, 22 match exactly. 38 differ, mostly `v8.1.10` → `v8.1.11`
(http-foundation, http-kernel, routing, config, dependency-injection,
security-*, form, validator, yaml, ...), and the contracts differ (`v3.7.x`
→ `v3.8.1`). It also installs 39 packages pinned nowhere here, among them
`easycorp/easyadmin-bundle v5.10.0`, `doctrine/doctrine-bundle`,
`doctrine/migrations`, `symfony/serializer`, `symfony/uid`,
`symfony/ux-turbo`, `symfony/web-link`, `twig/string-extra`, monolog,
phpdocumentor/phpstan type parsers and polyfills.

Recording this app's responses with its own lock is therefore a version
move for the shared packages (PLAN.md, "Version moves"). The v8.1.11
triage already done for http-foundation, routing and http-kernel (PLAN.md
M0) covers part of it. Until the owner decides (decision 10), each
comparison names the lock it was recorded with.

## Production boot

At the pinned commit the application does not boot with `APP_ENV=prod`:

```
In ArrayNode.php line 312:
  Unrecognized options "auto_generate_proxy_classes, proxy_dir" under
  "doctrine.orm.entity_managers.default". ...
```

The `when@prod` block of `config/packages/doctrine.yaml` still sets two
options that DoctrineBundle 3 removed. sulu/skeleton removed the same
options in its 8.1 upgrade ([sulu/skeleton#348](https://github.com/sulu/skeleton/pull/348)).
`dev` adds the web profiler (toolbar, `X-Debug-Token` headers) and `test`
swaps sessions for mock storage, so neither shows production behaviour.
Deleting just those two lines, in a copy under `oracle/out/`, makes `prod`
boot. The figures below come from that copy. How the oracle should run
this app is decision 10's open point.

## Inventory (prod, two-line overlay)

Produced in `oracle/out/apps/easyadmin-demo` (a `git archive` of the pin,
`composer install --no-scripts --ignore-platform-reqs` in `composer:2`
2.11.2, then `bin/console` in the oracle image with `APP_ENV=prod
APP_DEBUG=0`).

### Routes (`debug:router`): 142

| Group | Count | Notes |
|---|---|---|
| EasyAdmin `#[AdminRoute]`s, from the `easyadmin.routes` loader | 137 | 130 for 12 CRUD controllers (index, new, edit, detail, delete, batch-delete, autocomplete and render-filters on each, plus custom actions such as `mark-as-paid`, `publish`, `batch-approve`, CSV `export` and `print`); 2 for FeatureTourController, 1 for FixturesController, and 4 for EasyAdmin's own UiKitController |
| `admin` dashboard | 1 | `/{_locale}/admin`, `#[AdminDashboard]` |
| App `#[Route]` | 4 | `/` (preferred-language redirect), `/{_locale}/` (redirect to `admin`), `POST /{_locale}/demo-login` and `/{_locale}/logout` (both handled by the firewall) |

Every route carries `_locale: en|es|fr|de` (`framework.enabled_locales`).
Three add one more requirement: `feature: [a-z_]+` on
`admin_feature_tour_code`, and `name: [a-z0-9_]+` on the two UI-kit
pages. `dev` adds 15 profiler
and error-preview routes (157 in all).

### Services and listeners

- 872 service definitions and 189 aliases in `prod` (1,030 and 195 in `dev`).
- `kernel.request`, in order: DebugHandlersListener (2048), Turbo
  RequestListener (256), ValidateRequestListener (256), Doctrine
  IdleConnection Listener (192), SessionListener (128),
  LocaleListener::setDefaultLocale (100), RouterListener (32),
  LocaleListener (16), LocaleAwareListener (15), FirewallListener ×2 (8),
  `App\EventSubscriber\DemoAutoLoginSubscriber` (7), EasyAdmin
  AdminRouterSubscriber (1).
- `DemoAutoLoginSubscriber` logs every visitor in as the admin user on
  safe requests in the `main` firewall. It reads the session and loads the
  `User` entity, so even `GET /` crosses Security, the session and Doctrine.

### Application code

| Area | Use |
|---|---|
| PHP sources | 68 files in `src/`; imports: EasyAdmin 216, Security 17, HttpFoundation 16, Doctrine Collections 14, Translation contracts 13 / component 12, Zenstruck Foundry 10, Doctrine ORM mapping 10 |
| Attributes | `ORM\*` (72 `Column`, 10 entities with associations), 26 `#[AdminRoute]`, 4 `#[Route]`, 4 `#[Autowire]`, 2 `#[AsTwigFunction]`, `#[MapEntity]`, `#[IsGranted]`, `#[AdminDashboard]`, `#[AsFixture]` |
| PHP enums | 5 backed enums (`OrderStatus`, `ProductStatus`, `ReviewStatus`, `CustomerSource`, `DemoUser`) |
| Security | entity user provider (`email`), role hierarchy, lazy `main` firewall, custom `DemoUserAuthenticator`, logout, `switch_user`, `ProductVoter` |
| Templates | 34 app Twig templates overriding or extending EasyAdmin's; 24 use `<twig:...>` components (`ea:Badge`, `ea:Button`, `ea:Icon`, `ea:Modal`, `ea:Flag`, the app's `DashboardFieldset`); functions `path()`, `asset()`, `csrf_token()` |
| Translations | `translations/messages+intl-icu.{de,en,es,fr}.php`: PHP-array catalogues with ICU MessageFormat |
| Configuration | 18 `config/packages/*.yaml`, `services.yaml`, 4 route files, `config/reference.php` |
| Assets | AssetMapper and importmap; Stimulus (`csrf_protection_controller.js`), Turbo, Lucide icons through ux-icons |
| Responses | HTML pages, redirects, flash messages, a CSV export (`OrderCrudController::exportCsv`) |

### Persistence, queues and integrations

- **Persistence:** Doctrine ORM 3.7 / DBAL 4.5 on SQLite
  (`var/data.db`). `migrations/` is empty, and demo data comes from
  Zenstruck Foundry stories (`App\Story\InitialStateStory`).
  `FixturesController` (`/admin/regenerate-fixtures`, `ROLE_ADMIN`) resets
  the database and reloads the stories at run time, through a
  `require-dev` package.
- **Queues:** none (no Messenger configured).
- **Mail:** none (`symfony/mime` is installed; no Mailer).
- **External integrations:** none. `symfony/http-client` is required, but
  nothing in `src/` uses it.

## Ranked component work

Ranked by the Symfony demo plus this inventory. "W1" is the first workflow
to record from this app.

**W1 (first recorded workflow):** `GET /` with `Accept-Language`
variations (`en`, `fr`, `de-AT`, an unsupported language, none), followed
by its `/{_locale}/` redirect. It runs once on an empty database and once
after the stories are loaded. The recording keeps status, `Location`,
`Set-Cookie` and `Cache-Control`. Whether the auto-login starts a session
on these redirects is what the recording will show; it is not assumed here.

| Rank | Components |
|---|---|
| Required for W1 | Routing (requirements, `redirectToRoute` via UrlGenerator); HttpFoundation (`getPreferredLanguage`, `RedirectResponse`, sessions and their cookie); HttpKernel's Session, Locale, Router and ValidateRequest listeners; DI parameters (`%kernel.enabled_locales%`); Security (lazy firewall, `Security::login()`, session token storage); Doctrine `findOneBy` on `User` (SQLite) |
| Required later | EasyAdmin bundle (dashboard, CRUD, filters, batch actions; 341 PHP files, 191 templates and 355 test files at v5.10.0); Twig with TwigComponent and ux-icons; Form; Validator; Translation with ICU MessageFormat and PHP-array catalogues; Intl; Doctrine associations and QueryBuilder; CSRF; voters; `switch_user`; StreamedResponse (CSV); AssetMapper/importmap; Turbo's request listener |
| Not yet demanded | HttpClient, Serializer, Uid, Mime, Messenger, Mailer: installed or available, but not used by this app's code or the demo's recorded flows |
| Out of scope for comparison | `/admin/regenerate-fixtures` (Zenstruck Foundry, a dev dependency, at run time); profiler and error-preview routes (`dev` only) |

Fixtures recorded from PHP, manually ported Rust flows and genuinely
converted flows are labelled separately as each one is added (PLAN.md,
M1-D).
