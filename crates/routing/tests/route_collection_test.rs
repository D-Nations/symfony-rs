//! RouteCollectionTest.php at symfony/routing
//! f3fefc6cad3748d538be970ca81b71f06d7aa8b3. Names and assertions are
//! PHPUnit's. A collection owns its routes, so a route PHP keeps a handle
//! to (`$routea`) is read back through `get()`, and `assertSame()` on
//! routes compares values (a copy taken before the route was added).
//! The two `FileResource` fixtures (empty files) are in tests/fixtures.
//!
//! Upstream: symfony/routing Tests/RouteCollectionTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::{TestResult, list, s, strings};
use php_value::{PhpArray, PhpString};
use symfony_config::resource::{FileResource, ResourceInterface};
use symfony_routing::{
    exception::Error,
    route::{Arguments, Route},
    route_collection::RouteCollection,
};

fn route(path: &str) -> Result<Route, Error> {
    Route::new(path)
}

/// `$collection->all()` as owned pairs.
fn all(collection: &RouteCollection) -> Vec<(PhpString, Route)> {
    collection
        .all()
        .into_iter()
        .map(|(n, r)| (n.clone(), r.clone()))
        .collect()
}

fn named(pairs: &[(&str, &Route)]) -> Vec<(PhpString, Route)> {
    pairs.iter().map(|(n, r)| (s(n), (*r).clone())).collect()
}

/// `$collection->get($name)->getPath()`.
fn path(collection: &RouteCollection, name: &str) -> Result<Option<PhpString>, Error> {
    Ok(collection.get(name.as_bytes())?.map(|r| r.path().clone()))
}

fn fixture(name: &str) -> Result<Arc<dyn ResourceInterface>, Box<dyn std::error::Error>> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    Ok(Arc::new(FileResource::new(path.as_bytes())?))
}

/// `assertEquals()` on resources: same class and string form.
fn resource_ids(resources: &[Arc<dyn ResourceInterface>]) -> Vec<(String, Vec<u8>)> {
    resources
        .iter()
        .map(|r| (r.class().to_owned(), r.to_string()))
        .collect()
}

#[test]
fn testRoute() -> TestResult {
    let mut collection = RouteCollection::new();
    let route = route("/foo")?;
    collection.add("foo", route.clone(), 0);
    assert_eq!(
        named(&[("foo", &route)]),
        all(&collection),
        "->add() adds a route"
    );
    assert_eq!(
        Some(&route),
        collection.get(b"foo")?,
        "->get() returns a route by name"
    );
    assert_eq!(
        None,
        collection.get(b"bar")?,
        "->get() returns null if a route does not exist"
    );
    Ok(())
}

#[test]
fn testOverriddenRoute() -> TestResult {
    let mut collection = RouteCollection::new();
    collection.add("foo", route("/foo")?, 0);
    collection.add("foo", route("/foo1")?, 0);

    assert_eq!(Some(s("/foo1")), path(&collection, "foo")?);
    Ok(())
}

#[test]
fn testDeepOverriddenRoute() -> TestResult {
    let mut collection = RouteCollection::new();
    collection.add("foo", route("/foo")?, 0);

    let mut collection1 = RouteCollection::new();
    collection1.add("foo", route("/foo1")?, 0);

    let mut collection2 = RouteCollection::new();
    collection2.add("foo", route("/foo2")?, 0);

    collection1.add_collection(collection2);
    assert_eq!(Some(s("/foo2")), path(&collection1, "foo")?);
    collection.add_collection(collection1);

    assert_eq!(Some(s("/foo2")), path(&collection, "foo")?);
    Ok(())
}

#[test]
fn testIterator() -> TestResult {
    let mut collection = RouteCollection::new();
    collection.add("foo", route("/foo")?, 0);

    let mut collection1 = RouteCollection::new();
    let bar = route("/bar")?;
    let foo = route("/foo-new")?;
    collection1.add("bar", bar.clone(), 0);
    collection1.add("foo", foo.clone(), 0);
    collection.add_collection(collection1);
    let last = route("/last")?;
    collection.add("last", last.clone(), 0);

    assert_eq!(
        named(&[("bar", &bar), ("foo", &foo), ("last", &last)]),
        all(&collection)
    );
    Ok(())
}

#[test]
fn testCount() -> TestResult {
    let mut collection = RouteCollection::new();
    collection.add("foo", route("/foo")?, 0);

    let mut collection1 = RouteCollection::new();
    collection1.add("bar", route("/bar")?, 0);
    collection.add_collection(collection1);

    assert_eq!(2, collection.count());
    Ok(())
}

#[test]
fn testAddCollection() -> TestResult {
    let mut collection = RouteCollection::new();
    collection.add("foo", route("/foo")?, 0);

    let mut collection1 = RouteCollection::new();
    let bar = route("/bar")?;
    let foo = route("/foo-new")?;
    collection1.add("bar", bar.clone(), 0);
    collection1.add("foo", foo.clone(), 0);

    let mut collection2 = RouteCollection::new();
    let grandchild = route("/grandchild")?;
    collection2.add("grandchild", grandchild.clone(), 0);

    collection1.add_collection(collection2);
    collection.add_collection(collection1);
    let last = route("/last")?;
    collection.add("last", last.clone(), 0);

    assert_eq!(
        named(&[
            ("bar", &bar),
            ("foo", &foo),
            ("grandchild", &grandchild),
            ("last", &last)
        ]),
        all(&collection),
        "->addCollection() imports routes of another collection, overrides if necessary and adds them at the end"
    );
    Ok(())
}

#[test]
fn testAddCollectionWithResources() -> TestResult {
    let mut collection = RouteCollection::new();
    let foo = fixture("empty.yml")?;
    collection.add_resource(foo.clone());
    let mut collection1 = RouteCollection::new();
    let foo1 = fixture("file_resource.yml")?;
    collection1.add_resource(foo1.clone());
    collection.add_collection(collection1);
    assert_eq!(
        resource_ids(&[foo, foo1]),
        resource_ids(&collection.resources()),
        "->addCollection() merges resources"
    );
    Ok(())
}

#[test]
fn testAddDefaultsAndRequirementsAndOptions() -> TestResult {
    let mut collection = RouteCollection::new();
    collection.add("foo", route("/{placeholder}")?, 0);
    let mut collection1 = RouteCollection::new();
    collection1.add(
        "bar",
        Route::with(
            "/{placeholder}",
            Arguments {
                defaults: strings(&[("_controller", "fixed"), ("placeholder", "default")]),
                requirements: strings(&[("placeholder", ".+")]),
                options: strings(&[("option", "value")]),
                ..Arguments::default()
            },
        )?,
        0,
    );
    collection.add_collection(collection1);

    let get = |c: &RouteCollection, name: &str| -> Result<Route, Box<dyn std::error::Error>> {
        c.get(name.as_bytes())?
            .cloned()
            .ok_or_else(|| format!("no route {name}").into())
    };

    collection.add_defaults(&strings(&[("placeholder", "new-default")]))?;
    assert_eq!(
        strings(&[("placeholder", "new-default")]),
        *get(&collection, "foo")?.defaults(),
        "->addDefaults() adds defaults to all routes"
    );
    assert_eq!(
        strings(&[("_controller", "fixed"), ("placeholder", "new-default")]),
        *get(&collection, "bar")?.defaults(),
        "->addDefaults() adds defaults to all routes and overwrites existing ones"
    );

    collection.add_requirements(&strings(&[("placeholder", r"\d+")]))?;
    assert_eq!(
        strings(&[("placeholder", r"\d+")]),
        *get(&collection, "foo")?.requirements(),
        "->addRequirements() adds requirements to all routes"
    );
    assert_eq!(
        strings(&[("placeholder", r"\d+")]),
        *get(&collection, "bar")?.requirements(),
        "->addRequirements() adds requirements to all routes and overwrites existing ones"
    );

    collection.add_options(&strings(&[("option", "new-value")]));
    // assertEquals() on arrays ignores key order
    let mut expected = strings(&[
        ("option", "new-value"),
        (
            "compiler_class",
            "Symfony\\Component\\Routing\\RouteCompiler",
        ),
    ]);
    let mut actual = get(&collection, "bar")?.options().clone();
    expected.ksort();
    actual.ksort();
    assert_eq!(
        expected, actual,
        "->addOptions() adds options to all routes and overwrites existing ones"
    );
    Ok(())
}

#[test]
fn testAddPrefix() -> TestResult {
    let mut collection = RouteCollection::new();
    collection.add("foo", route("/foo")?, 0);
    let mut collection2 = RouteCollection::new();
    collection2.add("bar", route("/bar")?, 0);
    collection.add_collection(collection2);
    let none = PhpArray::new;
    collection.add_prefix(b" / ", &none(), &none())?;
    assert_eq!(
        Some(s("/foo")),
        path(&collection, "foo")?,
        "->addPrefix() trims the prefix and a single slash has no effect"
    );
    collection.add_prefix(
        b"/{admin}",
        &strings(&[("admin", "admin")]),
        &strings(&[("admin", r"\d+")]),
    )?;
    assert_eq!(
        Some(s("/{admin}/foo")),
        path(&collection, "foo")?,
        "->addPrefix() adds a prefix to all routes"
    );
    assert_eq!(
        Some(s("/{admin}/bar")),
        path(&collection, "bar")?,
        "->addPrefix() adds a prefix to all routes"
    );
    let route = |name: &str| collection.get(name.as_bytes()).ok().flatten().cloned();
    assert_eq!(
        Some(strings(&[("admin", "admin")])),
        route("foo").map(|r| r.defaults().clone()),
        "->addPrefix() adds defaults to all routes"
    );
    assert_eq!(
        Some(strings(&[("admin", "admin")])),
        route("bar").map(|r| r.defaults().clone()),
        "->addPrefix() adds defaults to all routes"
    );
    assert_eq!(
        Some(strings(&[("admin", r"\d+")])),
        route("foo").map(|r| r.requirements().clone()),
        "->addPrefix() adds requirements to all routes"
    );
    assert_eq!(
        Some(strings(&[("admin", r"\d+")])),
        route("bar").map(|r| r.requirements().clone()),
        "->addPrefix() adds requirements to all routes"
    );
    collection.add_prefix(b"0", &none(), &none())?;
    assert_eq!(
        Some(s("/0/{admin}/foo")),
        path(&collection, "foo")?,
        "->addPrefix() ensures a prefix must start with a slash and must not end with a slash"
    );
    collection.add_prefix(b"/ /", &none(), &none())?;
    assert_eq!(
        Some(s("/ /0/{admin}/foo")),
        path(&collection, "foo")?,
        "->addPrefix() can handle spaces if desired"
    );
    assert_eq!(
        Some(s("/ /0/{admin}/bar")),
        path(&collection, "bar")?,
        "the route pattern of an added collection is in synch with the added prefix"
    );
    Ok(())
}

#[test]
fn testAddPrefixOverridesDefaultsAndRequirements() -> TestResult {
    let mut collection = RouteCollection::new();
    collection.add("foo", route("/foo.{_format}")?, 0);
    collection.add(
        "bar",
        Route::with(
            "/bar.{_format}",
            Arguments {
                requirements: strings(&[("_format", "json")]),
                ..Arguments::default()
            },
        )?,
        0,
    );
    collection.add_prefix(
        b"/admin",
        &PhpArray::new(),
        &strings(&[("_format", "html")]),
    )?;

    let requirement = |name: &str| {
        collection
            .get(name.as_bytes())
            .ok()
            .flatten()
            .and_then(|r| r.requirement(b"_format").cloned())
    };
    assert_eq!(
        Some(s("html")),
        requirement("foo"),
        "->addPrefix() overrides existing requirements"
    );
    assert_eq!(
        Some(s("html")),
        requirement("bar"),
        "->addPrefix() overrides existing requirements"
    );
    Ok(())
}

#[test]
fn testResource() -> TestResult {
    let mut collection = RouteCollection::new();
    let foo = fixture("empty.yml")?;
    let bar = fixture("file_resource.yml")?;
    collection.add_resource(foo.clone());
    collection.add_resource(bar.clone());
    collection.add_resource(fixture("empty.yml")?);

    assert_eq!(
        resource_ids(&[foo, bar]),
        resource_ids(&collection.resources()),
        "->addResource() adds a resource and getResources() only returns unique ones by comparing the string representation"
    );
    Ok(())
}

#[test]
fn testUniqueRouteWithGivenName() -> TestResult {
    let mut collection1 = RouteCollection::new();
    collection1.add("foo", route("/old")?, 0);
    let mut collection2 = RouteCollection::new();
    let mut collection3 = RouteCollection::new();
    let new = route("/new")?;
    collection3.add("foo", new.clone(), 0);

    collection2.add_collection(collection3);
    collection1.add_collection(collection2);

    assert_eq!(
        Some(&new),
        collection1.get(b"foo")?,
        "->get() returns new route that overrode previous one"
    );
    // size of 1 because collection1 contains /new but not /old anymore
    assert_eq!(
        1,
        collection1.all().len(),
        "->addCollection() removes previous routes when adding new routes with the same name"
    );
    Ok(())
}

#[test]
fn testGet() -> TestResult {
    let mut collection1 = RouteCollection::new();
    let a = route("/a")?;
    collection1.add("a", a, 0);
    let mut collection2 = RouteCollection::new();
    let b = route("/b")?;
    collection2.add("b", b.clone(), 0);
    let collection2_copy = collection2.clone();
    collection1.add_collection(collection2);
    let c = route("/special")?;
    collection1.add("$péß^a|", c.clone(), 0);

    assert_eq!(
        Some(&b),
        collection1.get(b"b")?,
        "->get() returns correct route in child collection"
    );
    assert_eq!(
        Some(&c),
        collection1.get("$péß^a|".as_bytes())?,
        "->get() can handle special characters"
    );
    assert_eq!(
        None,
        collection2_copy.get(b"a")?,
        "->get() does not return the route defined in parent collection"
    );
    assert_eq!(
        None,
        collection1.get(b"non-existent")?,
        "->get() returns null when route does not exist"
    );
    assert_eq!(
        None,
        collection1.get(b"0")?,
        "->get() does not disclose internal child RouteCollection"
    );
    Ok(())
}

#[test]
fn testRemove() -> TestResult {
    let mut collection = RouteCollection::new();
    collection.add("foo", route("/foo")?, 0);

    let mut collection1 = RouteCollection::new();
    let bar = route("/bar")?;
    collection1.add("bar", bar.clone(), 0);
    collection.add_collection(collection1);
    let last = route("/last")?;
    collection.add("last", last.clone(), 0);
    collection.add_alias(b"alias_removed_when_removing_route_foo", b"foo")?;
    collection.add_alias(b"alias_directly_removed", b"bar")?;

    collection.remove(b"foo");
    assert_eq!(
        named(&[("bar", &bar), ("last", &last)]),
        all(&collection),
        "->remove() can remove a single route"
    );
    collection.remove(b"alias_directly_removed");
    assert_eq!(None, collection.alias(b"alias_directly_removed"));
    collection.remove_all(&[b"bar", b"last"]);
    assert_eq!(
        Vec::<(PhpString, Route)>::new(),
        all(&collection),
        "->remove() accepts an array and can remove multiple routes at once"
    );
    assert_eq!(
        None,
        collection.alias(b"alias_removed_when_removing_route_foo")
    );
    Ok(())
}

#[test]
fn testSetHost() -> TestResult {
    let mut collection = RouteCollection::new();
    let routea = route("/a")?;
    let routeb = Route::with(
        "/b",
        Arguments {
            host: s("{locale}.example.net"),
            ..Arguments::default()
        },
    )?;
    collection.add("a", routea, 0);
    collection.add("b", routeb, 0);

    collection.set_host(b"{locale}.example.com", &PhpArray::new(), &PhpArray::new())?;

    let host = |name: &str| {
        collection
            .get(name.as_bytes())
            .ok()
            .flatten()
            .map(|r| r.host().clone())
    };
    assert_eq!(Some(s("{locale}.example.com")), host("a"));
    assert_eq!(Some(s("{locale}.example.com")), host("b"));
    Ok(())
}

#[test]
fn testSetCondition() -> TestResult {
    let mut collection = RouteCollection::new();
    let routea = route("/a")?;
    let routeb = Route::with(
        "/b",
        Arguments {
            host: s("{locale}.example.net"),
            condition: s("context.getMethod() == \"GET\""),
            ..Arguments::default()
        },
    )?;
    collection.add("a", routea, 0);
    collection.add("b", routeb, 0);

    collection.set_condition(b"context.getMethod() == \"POST\"");

    let condition = |name: &str| {
        collection
            .get(name.as_bytes())
            .ok()
            .flatten()
            .map(|r| r.condition().clone())
    };
    assert_eq!(Some(s("context.getMethod() == \"POST\"")), condition("a"));
    assert_eq!(Some(s("context.getMethod() == \"POST\"")), condition("b"));
    Ok(())
}

#[test]
fn testClone() -> TestResult {
    let mut collection = RouteCollection::new();
    collection.add("a", route("/a")?, 0);
    collection.add(
        "b",
        Route::with(
            "/b",
            Arguments {
                defaults: strings(&[("placeholder", "default")]),
                requirements: strings(&[("placeholder", ".+")]),
                ..Arguments::default()
            },
        )?,
        0,
    );

    let cloned_collection = collection.clone();

    assert_eq!(2, cloned_collection.count());
    assert_eq!(collection.get(b"a")?, cloned_collection.get(b"a")?);
    assert!(!std::ptr::eq(
        collection.get(b"a")?.ok_or("a")?,
        cloned_collection.get(b"a")?.ok_or("a")?
    ));
    assert_eq!(collection.get(b"b")?, cloned_collection.get(b"b")?);
    assert!(!std::ptr::eq(
        collection.get(b"b")?.ok_or("b")?,
        cloned_collection.get(b"b")?.ok_or("b")?
    ));
    Ok(())
}

#[test]
fn testSetSchemes() -> TestResult {
    let mut collection = RouteCollection::new();
    let routea = Route::with(
        "/a",
        Arguments {
            schemes: vec![s("http")],
            ..Arguments::default()
        },
    )?;
    let routeb = route("/b")?;
    collection.add("a", routea, 0);
    collection.add("b", routeb, 0);

    collection.set_schemes(&["http", "https"]);

    let schemes = |name: &str| {
        collection
            .get(name.as_bytes())
            .ok()
            .flatten()
            .map(|r| r.schemes().clone())
    };
    assert_eq!(Some(list(&["http", "https"])), schemes("a"));
    assert_eq!(Some(list(&["http", "https"])), schemes("b"));
    Ok(())
}

#[test]
fn testSetMethods() -> TestResult {
    let mut collection = RouteCollection::new();
    let routea = Route::with(
        "/a",
        Arguments {
            methods: vec![s("GET"), s("POST")],
            ..Arguments::default()
        },
    )?;
    let routeb = route("/b")?;
    collection.add("a", routea, 0);
    collection.add("b", routeb, 0);

    collection.set_methods(&["PUT"]);

    let methods = |name: &str| {
        collection
            .get(name.as_bytes())
            .ok()
            .flatten()
            .map(|r| r.methods().clone())
    };
    assert_eq!(Some(list(&["PUT"])), methods("a"));
    assert_eq!(Some(list(&["PUT"])), methods("b"));
    Ok(())
}

#[test]
fn testAddNamePrefix() -> TestResult {
    let mut collection = RouteCollection::new();
    let foo = route("/foo")?;
    let bar = route("/bar")?;
    let api_foo = route("/api/foo")?;
    collection.add("foo", foo.clone(), 0);
    collection.add("bar", bar.clone(), 0);
    collection.add("api_foo", api_foo.clone(), 0);
    collection.add_name_prefix(b"api_")?;

    assert_eq!(Some(&foo), collection.get(b"api_foo")?);
    assert_eq!(Some(&bar), collection.get(b"api_bar")?);
    assert_eq!(Some(&api_foo), collection.get(b"api_api_foo")?);
    assert_eq!(None, collection.get(b"foo")?);
    assert_eq!(None, collection.get(b"bar")?);
    Ok(())
}

#[test]
fn testAddNamePrefixCanonicalRouteName() -> TestResult {
    let mut collection = RouteCollection::new();
    let canonical = |path: &str, name: &str| {
        Route::with(
            path,
            Arguments {
                defaults: strings(&[("_canonical_route", name)]),
                ..Arguments::default()
            },
        )
    };
    collection.add("foo", canonical("/foo", "foo")?, 0);
    collection.add("bar", canonical("/bar", "bar")?, 0);
    collection.add("api_foo", canonical("/api/foo", "api_foo")?, 0);
    collection.add_name_prefix(b"api_")?;

    let value = |name: &str| {
        collection
            .get(name.as_bytes())
            .ok()
            .flatten()
            .and_then(|r| r.default_value(b"_canonical_route").cloned())
    };
    assert_eq!(Some(php_value::Value::string("api_foo")), value("api_foo"));
    assert_eq!(Some(php_value::Value::string("api_bar")), value("api_bar"));
    assert_eq!(
        Some(php_value::Value::string("api_api_foo")),
        value("api_api_foo")
    );
    Ok(())
}

#[test]
fn testAddWithPriority() -> TestResult {
    let mut collection = RouteCollection::new();
    let foo = route("/foo")?;
    let bar = route("/bar")?;
    let baz = route("/baz")?;
    collection.add("foo", foo.clone(), 0);
    collection.add("bar", bar.clone(), 1);
    collection.add("baz", baz.clone(), 0);

    let expected = named(&[("bar", &bar), ("foo", &foo), ("baz", &baz)]);

    assert_eq!(expected, all(&collection));

    let mut collection2 = RouteCollection::new();
    let foo2 = route("/foo")?;
    let bar2 = route("/bar")?;
    let baz2 = route("/baz")?;
    collection2.add("foo2", foo2.clone(), 0);
    collection2.add("bar2", bar2.clone(), 1);
    collection2.add("baz2", baz2.clone(), 0);
    collection2.add_collection(collection);

    let expected = named(&[
        ("bar2", &bar2),
        ("bar", &bar),
        ("foo2", &foo2),
        ("baz2", &baz2),
        ("foo", &foo),
        ("baz", &baz),
    ]);

    assert_eq!(expected, all(&collection2));
    Ok(())
}

#[test]
fn testAddWithPriorityAndPrefix() -> TestResult {
    let mut collection3 = RouteCollection::new();
    let foo3 = route("/foo")?;
    let bar3 = route("/bar")?;
    let baz3 = route("/baz")?;
    collection3.add("foo3", foo3.clone(), 0);
    collection3.add("bar3", bar3.clone(), 1);
    collection3.add("baz3", baz3.clone(), 0);
    collection3.add_name_prefix(b"prefix_")?;

    let expected = named(&[
        ("prefix_bar3", &bar3),
        ("prefix_foo3", &foo3),
        ("prefix_baz3", &baz3),
    ]);

    assert_eq!(expected, all(&collection3));
    Ok(())
}

#[test]
fn testAddNamePrefixDoesNotBreakExternalAliases() -> TestResult {
    let mut collection = RouteCollection::new();
    collection.add("local_route", route("/local")?, 0);
    collection.add_alias(b"alias_to_local", b"local_route")?;
    collection.add_alias(b"alias_to_external", b"external_route")?;
    collection.add_name_prefix(b"prefix_")?;

    let id = |name: &str| collection.alias(name.as_bytes()).map(|a| a.id().clone());

    assert_eq!(
        Some(s("prefix_local_route")),
        id("prefix_alias_to_local"),
        "Alias to local route should have its target prefixed"
    );
    assert_eq!(
        Some(s("external_route")),
        id("prefix_alias_to_external"),
        "Alias to external route should NOT have its target prefixed"
    );
    Ok(())
}
