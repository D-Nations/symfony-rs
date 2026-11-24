//! The emitted container of symfony/demo: every service resolves, sharing
//! and scopes behave as converter contract 0.2 section 4 says, the cycle
//! through Twig's method calls behaves as in PHP, and the declared class
//! of each public service is the type PHP's container returns.
//!
//! Upstream: none

use std::{collections::BTreeMap, sync::Arc, thread};

use di_trial_demo::container::{Container, RequestScope, SERVICES, classes, parameter};
use di_trial_runtime::{Arg, Factory, Instance, Service, Value, describe};

fn app() -> Arc<Container> {
    let kernel = Arc::new(classes::app::Kernel(Instance::new(
        "kernel",
        "App\\Kernel",
        Factory::Constructor,
        Vec::new(),
    )));
    Container::new(kernel)
}

fn instance(service: &Service) -> &Instance {
    service.instance().expect("a placeholder")
}

#[test]
fn every_service_resolves() {
    let app = app();
    let scope = RequestScope::new(&app);
    assert_eq!(SERVICES.len(), 548);
    for info in SERVICES {
        let s = scope
            .resolve(info.id)
            .unwrap_or_else(|| panic!("{} does not resolve", info.id));
        if info.id != "service_container" {
            assert_eq!(instance(&s).class, info.class, "{}", info.id);
            assert_eq!(describe(&s), info.id);
        }
        if !info.request_scoped {
            assert!(app.resolve(info.id).is_some(), "{}", info.id);
        }
    }
}

#[test]
fn public_services_resolve_by_id_and_alias() {
    let app = app();
    let scope = RequestScope::new(&app);
    let public: Vec<_> = SERVICES.iter().filter(|s| s.public).collect();
    assert_eq!(public.len(), 35);
    for info in &public {
        assert!(scope.get(info.id).is_some(), "{}", info.id);
    }
    // Private services are not reachable through get(), as in PHP.
    assert!(scope.get("App\\Repository\\PostRepository").is_none());
    // Public aliases.
    let em = scope.get("doctrine.orm.entity_manager").expect("alias");
    let target = scope
        .get("doctrine.orm.default_entity_manager")
        .expect("service");
    assert!(Arc::ptr_eq(&em, &target));
}

/// PHP's `get_debug_type($container->get($id))` for every public service
/// (`scripts/record.sh di-trial`), against the class the definition
/// declares, which the generated accessor's type is.
#[test]
fn declared_classes_match_php() {
    let php: BTreeMap<String, String> =
        serde_json::from_str(include_str!("data/php_classes.json")).expect("JSON");
    let mut differences = Vec::new();
    for (id, php_type) in &php {
        if id == "service_container" {
            continue;
        }
        let declared = SERVICES
            .iter()
            .find(|s| s.id == id.as_str())
            .map(|s| s.class)
            .or_else(|| {
                // Public aliases: the type of their target.
                let app = app();
                let scope = RequestScope::new(&app);
                scope.get(id).and_then(|s| s.instance().map(|i| i.class))
            })
            .unwrap_or_else(|| panic!("{id} is not in the Rust container"));
        if declared != php_type {
            differences.push(format!("{id}: declared {declared}, PHP returns {php_type}"));
        }
    }
    // The definition's class is not always the runtime type: a factory
    // may return a subclass (cache.system's factory declares the
    // interface), and PhpDumper builds ServiceLocator definitions as
    // Argument\ServiceLocator. The converter needs the real types; the
    // container file alone does not give them.
    assert_eq!(
        differences,
        [
            "cache.system: declared Symfony\\Component\\Cache\\Adapter\\AdapterInterface, PHP returns Symfony\\Component\\Cache\\Adapter\\PhpFilesAdapter",
            "container.env_var_processors_locator: declared Symfony\\Component\\DependencyInjection\\ServiceLocator, PHP returns Symfony\\Component\\DependencyInjection\\Argument\\ServiceLocator",
        ],
        "differences between declared and returned types"
    );
}

#[test]
fn shared_services_are_built_once_and_others_each_time() {
    let app = app();
    let scope = RequestScope::new(&app);
    let a = scope.resolve("router").expect("router");
    let b = scope.resolve("router").expect("router");
    assert!(Arc::ptr_eq(&a, &b));
    let c = scope
        .resolve("App\\Twig\\Components\\BlogSearch")
        .expect("component");
    let d = scope
        .resolve("App\\Twig\\Components\\BlogSearch")
        .expect("component");
    assert!(!Arc::ptr_eq(&c, &d), "not shared: a new instance each time");
}

#[test]
fn request_scope_is_not_shared_between_requests() {
    let app = app();
    let one = RequestScope::new(&app);
    let two = RequestScope::new(&app);
    let info = |id: &str| SERVICES.iter().find(|s| s.id == id).copied().expect(id);
    // RequestStack keeps the current request: request-scoped.
    assert!(info("request_stack").request_scoped);
    assert!(!Arc::ptr_eq(
        &one.resolve("request_stack").expect("a"),
        &two.resolve("request_stack").expect("b")
    ));
    // A service that reaches no request-scoped one is shared by both.
    assert!(!info("App\\Utils\\Validator").request_scoped);
    assert!(Arc::ptr_eq(
        &one.resolve("App\\Utils\\Validator").expect("a"),
        &two.resolve("App\\Utils\\Validator").expect("b")
    ));
}

#[test]
fn constructor_arguments_follow_the_definition() {
    let app = app();
    let scope = RequestScope::new(&app);
    let s = scope
        .resolve("App\\EventSubscriber\\RedirectToPreferredLocaleSubscriber")
        .expect("subscriber");
    let i = instance(&s);
    let [
        Arg::Service(router),
        Arg::Array(locales),
        Arg::Value(default),
    ] = i.arguments.as_slice()
    else {
        panic!("unexpected arguments {:?}", i.arguments);
    };
    assert!(Arc::ptr_eq(
        router,
        &scope.resolve("router").expect("router")
    ));
    // %kernel.enabled_locales%, resolved at build time.
    assert_eq!(locales.len(), 30);
    assert_eq!(*default, Value::string("en"));
    assert!(matches!(parameter("kernel.enabled_locales"), Some(Value::Array(a)) if a.len() == 30));
}

#[test]
fn env_placeholders_stay_for_run_time() {
    let app = app();
    let scope = RequestScope::new(&app);
    let s = scope
        .resolve("doctrine.dbal.default_connection")
        .expect("connection");
    let found = format!("{:?}", instance(&s).arguments);
    assert!(found.contains("Env(resolve:DATABASE_URL)"), "{found}");
}

#[test]
fn closures_and_iterators_are_lazy() {
    let app = app();
    let scope = RequestScope::new(&app);
    // Each controller gets its locator through setContainer(); the
    // locator holds closures, so building the controller builds none of
    // the services it may later ask for.
    let c = scope
        .resolve("App\\Controller\\BlogController")
        .expect("controller");
    let calls = instance(&c).calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "setContainer");
    let Arg::Service(locator) = &calls[0].arguments[0] else {
        panic!("{:?}", calls[0].arguments);
    };
    let locator = instance(locator);
    assert!(matches!(
        locator.factory,
        Factory::Service {
            method: "withContext",
            ..
        }
    ));
    // services_resetter: ignore_uninitialized references resolve to
    // nothing until the service is built.
    let resetter = scope.resolve("services_resetter").expect("resetter");
    let Arg::Iterator(items) = &instance(&resetter).arguments[0] else {
        panic!();
    };
    let cache_app = items
        .iter()
        .find(|(k, _)| format!("{k:?}").contains("cache.app"))
        .map(|(_, a)| a.clone())
        .expect("cache.app");
    let Arg::Closure(get) = cache_app else {
        panic!()
    };
    // Building the resetter built none of the services it resets.
    assert!(scope.cache_app_if_built().is_none());
    assert!(get().is_none());
    let built = scope.cache_app();
    let got = get().expect("built now");
    assert!(Arc::ptr_eq(&got, &(built as Service)));
}

/// twig -> addExtension(new StimulusTwigExtension(stimulus.helper)) ->
/// stimulus.helper -> twig: allowed in PHP because the Environment is
/// stored before its method calls run. Each entry point gives one
/// instance of each.
#[test]
fn cycle_through_method_calls_works_from_both_ends() {
    for first in ["twig", "stimulus.helper"] {
        let app = app();
        let scope = RequestScope::new(&app);
        let _ = scope.resolve(first).expect(first);
        let twig = scope.resolve("twig").expect("twig");
        let helper = scope.resolve("stimulus.helper").expect("helper");
        let Arg::Service(helper_twig) = &instance(&helper).arguments[0] else {
            panic!();
        };
        assert!(Arc::ptr_eq(helper_twig, &twig), "{first}");
        // Twig's extension calls all ran, once.
        let calls = instance(&twig).calls();
        let extensions = calls.iter().filter(|c| c.method == "addExtension").count();
        assert!(extensions > 10, "{extensions}");
        let again = instance(&scope.resolve("twig").expect("twig"))
            .calls()
            .len();
        assert_eq!(again, calls.len());
    }
}

fn assert_send_sync<T: Send + Sync>() {}
fn assert_send<T: Send>(_: &T) {}

#[test]
fn concurrency_bounds_hold() {
    assert_send_sync::<Container>();
    assert_send_sync::<Arc<RequestScope>>();
    let app = app();
    let future = {
        let app = app.clone();
        async move {
            let scope = RequestScope::new(&app);
            let router = scope.resolve("router");
            std::future::ready(()).await;
            router.is_some()
        }
    };
    assert_send(&future);
    // Two requests on two threads, one application scope.
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let app = app.clone();
            thread::spawn(move || {
                let scope = RequestScope::new(&app);
                SERVICES.iter().all(|s| scope.resolve(s.id).is_some())
            })
        })
        .collect();
    for h in handles {
        assert!(h.join().expect("thread"));
    }
}
