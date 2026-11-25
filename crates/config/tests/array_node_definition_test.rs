//! ArrayNodeDefinitionTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's; provider rows are upstream's, written as
//! literals. `getField()` (`ReflectionProperty`) is `property()`, and
//! the `children` property `getChildNodeDefinitions()`. `assertEquals()`
//! of two definitions is `equals()`.
//!
//! Upstream: symfony/config Tests/Definition/Builder/ArrayNodeDefinitionTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

mod common;

use common::{TestResult, expect_class, expect_exception};
use php_value::{Key, PhpArray, Value, php_array, vectors::phpunit_equals};
use symfony_config::{
    Class, Error, Processor,
    definition::{
        builder::{ArrayNodeDefinition, Builtin, NodeDefinition, ScalarNodeDefinition, class},
        prototyped_array_node,
    },
};

fn child(node: &ArrayNodeDefinition, name: &str) -> Result<NodeDefinition, Error> {
    node.get_child_node_definitions()
        .into_iter()
        .find(|(k, _)| *k == Key::from(name))
        .map(|(_, d)| d)
        .ok_or_else(|| Error::new(Class::Error, format!("no child {name}")))
}

fn attribute(definition: &NodeDefinition, name: &str) -> Result<Value, Error> {
    Ok(definition
        .property("attributes")?
        .as_array()
        .and_then(|a| a.get(&Key::from(name)).cloned())
        .unwrap_or(Value::Null))
}

#[track_caller]
fn assert_equals(expected: Value, actual: Value) {
    assert!(
        phpunit_equals(&expected, &actual),
        "expected {expected:?}, got {actual:?}"
    );
}

fn array(value: PhpArray) -> Value {
    Value::Array(value)
}

#[test]
fn testAppendingSomeNode() -> TestResult {
    let parent = ArrayNodeDefinition::new("root");
    let child = ScalarNodeDefinition::new("child");

    parent
        .children()
        .scalar_node("foo")?
        .end()
        .scalar_node("bar")?
        .end()
        .end()
        .append(&child);

    let children = parent.get_child_node_definitions();
    assert_eq!(children.len(), 3);
    assert!(children.iter().any(|(_, d)| d.same(&child)));
    Ok(())
}

fn prototype_node_specific_call(node: &ArrayNodeDefinition, method: &str) -> Result<(), Error> {
    match method {
        "defaultValue" => node.default_value(array(PhpArray::new())).map(drop),
        "addDefaultChildrenIfNoneSet" => {
            node.add_default_children_if_none_set(Value::Null).map(drop)
        }
        "requiresAtLeastOneElement" => {
            node.requires_at_least_one_element();
            Ok(())
        }
        "cannotBeEmpty" => node.cannot_be_empty().map(drop),
        "useAttributeAsKey" => {
            node.use_attribute_as_key("foo", true);
            Ok(())
        }
        other => panic!("unknown method {other}"),
    }
}

#[test]
fn testPrototypeNodeSpecificOption() -> TestResult {
    for method in [
        "defaultValue",
        "addDefaultChildrenIfNoneSet",
        "requiresAtLeastOneElement",
        "cannotBeEmpty",
        "useAttributeAsKey",
    ] {
        let node = ArrayNodeDefinition::new("root");
        prototype_node_specific_call(&node, method)?;
        expect_class(node.get_node(false), Class::InvalidDefinition);
    }
    Ok(())
}

#[test]
fn testConcreteNodeSpecificOption() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.add_defaults_if_not_set().prototype("array")?;
    expect_class(node.get_node(false), Class::InvalidDefinition);
    Ok(())
}

#[test]
fn testPrototypeNodesCantHaveADefaultValueWhenUsingDefaultChildren() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.default_value(array(PhpArray::new()))?
        .add_default_children_if_none_set("foo")?
        .prototype("array")?;
    expect_class(node.get_node(false), Class::InvalidDefinition);
    Ok(())
}

#[test]
fn testPrototypedArrayNodeDefaultWhenUsingDefaultChildren() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.add_default_children_if_none_set(Value::Null)?
        .prototype("array")?;
    let tree = node.get_node(false)?;
    assert_equals(
        array(PhpArray::list([array(PhpArray::new())])),
        tree.get_default_value()?,
    );
    Ok(())
}

fn provide_prototyped_array_node_defaults() -> Vec<(Value, bool, bool, PhpArray)> {
    let empty = || array(PhpArray::new());
    vec![
        (Value::Null, true, false, PhpArray::list([empty()])),
        (
            Value::Int(2),
            true,
            false,
            PhpArray::list([empty(), empty()]),
        ),
        (
            Value::from("2"),
            false,
            true,
            php_array!["2" => PhpArray::new()],
        ),
        (
            Value::from("foo"),
            false,
            true,
            php_array!["foo" => PhpArray::new()],
        ),
        (
            array(PhpArray::list([Value::from("foo")])),
            false,
            true,
            php_array!["foo" => PhpArray::new()],
        ),
        (
            array(PhpArray::list([Value::from("foo"), Value::from("bar")])),
            false,
            true,
            php_array!["foo" => PhpArray::new(), "bar" => PhpArray::new()],
        ),
    ]
}

#[test]
fn testPrototypedArrayNodeDefault() -> TestResult {
    for (
        args,
        should_throw_when_using_attr_as_key,
        should_throw_when_not_using_attr_as_key,
        defaults,
    ) in provide_prototyped_array_node_defaults()
    {
        let node = ArrayNodeDefinition::new("root");
        node.add_default_children_if_none_set(args.clone())?
            .prototype("array")?;

        match node.get_node(false) {
            Ok(tree) => {
                assert!(!should_throw_when_not_using_attr_as_key);
                assert_equals(array(defaults.clone()), tree.get_default_value()?);
            }
            Err(e) if *e.class() == Class::InvalidDefinition => {
                assert!(should_throw_when_not_using_attr_as_key);
            }
            Err(e) => return Err(e.into()),
        }

        let node = ArrayNodeDefinition::new("root");
        node.use_attribute_as_key("attr", true)
            .add_default_children_if_none_set(args)?
            .prototype("array")?;

        match node.get_node(false) {
            Ok(tree) => {
                assert!(!should_throw_when_using_attr_as_key);
                assert_equals(array(defaults), tree.get_default_value()?);
            }
            Err(e) if *e.class() == Class::InvalidDefinition => {
                assert!(should_throw_when_using_attr_as_key);
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

#[test]
fn testNestedPrototypedArrayNodes() -> TestResult {
    let node_definition = ArrayNodeDefinition::new("root");
    node_definition
        .add_default_children_if_none_set(Value::Null)?
        .prototype("array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .prototype("array")?;
    let node = node_definition.get_node(false)?;

    assert!(node.is_instance_of(prototyped_array_node::CLASS));
    assert!(
        node.get_prototype()?
            .is_instance_of(prototyped_array_node::CLASS)
    );
    Ok(())
}

fn enableable_node() -> Result<ArrayNodeDefinition, Error> {
    let node = ArrayNodeDefinition::new("root");
    node.can_be_enabled(None)?
        .children()
        .scalar_node("foo")?
        .default_value(Value::from("bar"))?
        .end();
    Ok(node)
}

#[test]
fn testEnabledNodeDefaults() -> TestResult {
    let node = enableable_node()?;
    assert_equals(
        array(php_array!["enabled" => false, "foo" => "bar"]),
        node.get_node(false)?.get_default_value()?,
    );
    Ok(())
}

fn get_enableable_node_fixtures() -> Vec<(PhpArray, PhpArray, &'static str)> {
    vec![
        (
            php_array!["enabled" => true, "foo" => "bar"],
            PhpArray::list([Value::Bool(true)]),
            "true enables an enableable node",
        ),
        (
            php_array!["enabled" => true, "foo" => "bar"],
            PhpArray::list([Value::Null]),
            "null enables an enableable node",
        ),
        (
            php_array!["enabled" => true, "foo" => "bar"],
            PhpArray::list([array(php_array!["enabled" => true])]),
            "An enableable node can be enabled",
        ),
        (
            php_array!["enabled" => true, "foo" => "baz"],
            PhpArray::list([array(php_array!["foo" => "baz"])]),
            "any configuration enables an enableable node",
        ),
        (
            php_array!["enabled" => false, "foo" => "baz"],
            PhpArray::list([array(php_array!["foo" => "baz", "enabled" => false])]),
            "An enableable node can be disabled",
        ),
        (
            php_array!["enabled" => false, "foo" => "bar"],
            PhpArray::list([Value::Bool(false)]),
            "false disables an enableable node",
        ),
    ]
}

#[test]
fn testTrueEnableEnabledNode() -> TestResult {
    for (expected, config, message) in get_enableable_node_fixtures() {
        let processor = Processor::new();
        let node = enableable_node()?;
        let actual = processor.process(&node.get_node(false)?, &config)?;
        assert!(
            phpunit_equals(&array(expected), &array(actual)),
            "{message}"
        );
    }
    Ok(())
}

#[test]
fn testCanBeEnabledWithInfo() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.can_be_enabled(Some("Some info about disabling this node"))?;

    let child = child(&node, "enabled")?;
    assert_equals(
        Value::from("Some info about disabling this node"),
        attribute(&child, "info")?,
    );
    Ok(())
}

#[test]
fn testCanBeDisabled() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.can_be_disabled(None)?;

    assert!(node.property("addDefaults")?.identical(&Value::Bool(true)));
    assert_equals(
        array(php_array!["enabled" => false]),
        node.property("falseEquivalent")?,
    );
    assert_equals(
        array(php_array!["enabled" => true]),
        node.property("trueEquivalent")?,
    );
    assert_equals(
        array(php_array!["enabled" => true]),
        node.property("nullEquivalent")?,
    );

    let enabled_node = child(&node, "enabled")?;
    assert!(
        enabled_node
            .property("default")?
            .identical(&Value::Bool(true))
    );
    assert!(
        enabled_node
            .property("defaultValue")?
            .identical(&Value::Bool(true))
    );
    Ok(())
}

#[test]
fn testCanBeDisabledWithInfo() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.can_be_disabled(Some("Some info about disabling this node"))?;

    let child = child(&node, "enabled")?;
    assert_equals(
        Value::from("Some info about disabling this node"),
        attribute(&child, "info")?,
    );
    Ok(())
}

#[test]
fn testIgnoreExtraKeys() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    assert!(
        node.property("ignoreExtraKeys")?
            .identical(&Value::Bool(false))
    );

    let result = node.ignore_extra_keys(true);

    assert!(node.equals(&result));
    assert!(
        node.property("ignoreExtraKeys")?
            .identical(&Value::Bool(true))
    );
    Ok(())
}

#[test]
fn testNormalizeKeys() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    assert!(
        node.property("normalizeKeys")?
            .identical(&Value::Bool(true))
    );

    let result = node.normalize_keys(false);

    assert!(node.equals(&result));
    assert!(
        node.property("normalizeKeys")?
            .identical(&Value::Bool(false))
    );
    Ok(())
}

#[test]
fn testUnsetChild() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.children()
        .scalar_node("value")?
        .before_normalization()
        .if_true_with(|value| !symfony_config::php::truthy(value))
        .then_unset()
        .end()?
        .end()
        .end();

    assert!(
        array(PhpArray::new()).identical(
            &node
                .get_node(false)?
                .normalize(array(php_array!["value" => Value::Null]))?
        )
    );
    Ok(())
}

#[test]
fn testPrototypeVariable() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    assert!(
        node.prototype("variable")?
            .equals(&node.variable_prototype()?)
    );
    Ok(())
}

#[test]
fn testPrototypeScalar() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    assert!(node.prototype("scalar")?.equals(&node.scalar_prototype()?));
    Ok(())
}

#[test]
fn testPrototypeBoolean() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    assert!(
        node.prototype("boolean")?
            .equals(&node.boolean_prototype()?)
    );
    Ok(())
}

#[test]
fn testPrototypeString() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    assert!(node.prototype("string")?.equals(&node.string_prototype()?));
    Ok(())
}

#[test]
fn testPrototypeInteger() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    assert!(
        node.prototype("integer")?
            .equals(&node.integer_prototype()?)
    );
    Ok(())
}

#[test]
fn testPrototypeFloat() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    assert!(node.prototype("float")?.equals(&node.float_prototype()?));
    Ok(())
}

#[test]
fn testPrototypeArray() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    assert!(node.prototype("array")?.equals(&node.array_prototype()?));
    Ok(())
}

#[test]
fn testPrototypeEnum() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    assert!(node.prototype("enum")?.equals(&node.enum_prototype()?));
    Ok(())
}

#[test]
fn testPrototypedArrayAllowsNullDefault() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.default_value(Value::Null)?.prototype("array")?;

    let tree = node.get_node(false)?;
    assert!(tree.get_default_value()?.is_null());
    Ok(())
}

#[test]
fn testPrototypedArrayRejectsNonArrayDefaultValue() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.default_value(Value::from("not-an-array"))?
        .prototype("array")?;

    expect_exception(
        node.get_node(false),
        Class::InvalidArgument,
        "the default value of an array node has to be an array or null",
    );
    Ok(())
}

type Builder = fn(&ArrayNodeDefinition) -> Result<ArrayNodeDefinition, Error>;

fn provide_enabled_state_after_merging() -> Vec<(&'static str, PhpArray, PhpArray, Builder)> {
    let enabled: Builder = |node| node.can_be_enabled(None);
    let disabled: Builder = |node| node.can_be_disabled(None);
    vec![
        (
            "canBeEnabled keeps explicit true",
            php_array!["enabled" => true],
            php_array!["enabled" => true, "foo" => "baz"],
            enabled,
        ),
        (
            "canBeDisabled keeps explicit false",
            php_array!["enabled" => false],
            php_array!["enabled" => false, "foo" => "baz"],
            disabled,
        ),
        (
            "canBeDisabled keeps explicit true",
            php_array!["enabled" => true],
            php_array!["enabled" => true, "foo" => "baz"],
            disabled,
        ),
    ]
}

#[test]
fn testEnabledStateIsPreservedAcrossMergedConfigs() -> TestResult {
    for (name, first_config, expected, builder) in provide_enabled_state_after_merging() {
        let processor = Processor::new();
        let node = ArrayNodeDefinition::new("root");
        builder(&node)?
            .children()
            .scalar_node("foo")?
            .default_value(Value::from("bar"))?
            .end()
            .end();

        let actual = processor.process(
            &node.get_node(false)?,
            &PhpArray::list([array(first_config), array(php_array!["foo" => "baz"])]),
        )?;
        assert!(array(expected).identical(&array(actual)), "{name}");
    }
    Ok(())
}

#[test]
fn testRequiresAtLeastOneElement() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.requires_at_least_one_element().integer_prototype()?;

    node.get_node(false)?
        .finalize(array(PhpArray::list([Value::Int(1)])))?;
    Ok(())
}

#[test]
fn testCannotBeEmpty() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.cannot_be_empty()?.integer_prototype()?;

    expect_exception(
        node.get_node(false)?.finalize(array(PhpArray::new())),
        Class::InvalidConfiguration,
        "The path \"root\" should have at least 1 element(s) defined.",
    );
    Ok(())
}

#[test]
fn testSetDeprecated() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.children()
        .array_node("foo", None)?
        .set_deprecated(
            "vendor/package",
            "1.1",
            "The \"%path%\" node is deprecated.",
        )
        .end()
        .end();
    let deprecated_node = node
        .get_node(false)?
        .get_children()?
        .into_iter()
        .find(|(k, _)| *k == Key::from("foo"))
        .map(|(_, n)| n)
        .ok_or("no foo")?;

    assert!(deprecated_node.is_deprecated());
    let deprecation = deprecated_node
        .get_deprecation(&deprecated_node.get_name(), &deprecated_node.get_path())?;
    assert_eq!(deprecation.message, b"The \"root.foo\" node is deprecated.");
    assert_eq!(deprecation.package, b"vendor/package");
    assert_eq!(deprecation.version, b"1.1");
    Ok(())
}

#[test]
fn testCannotBeEmptyOnConcreteNode() -> TestResult {
    let node = ArrayNodeDefinition::new("root");
    node.cannot_be_empty()?;

    expect_exception(
        node.get_node(false)
            .and_then(|n| n.finalize(array(PhpArray::new()))),
        Class::InvalidDefinition,
        "->cannotBeEmpty() is not applicable to concrete nodes at path \"root\"",
    );
    Ok(())
}

#[test]
fn testFindShouldThrowExceptionIfNodeDoesNotExistInRootNode() -> TestResult {
    let root_node = ArrayNodeDefinition::new("root");
    root_node
        .children()
        .array_node("social_media_channels", None)?
        .end()
        .end();

    expect_exception(
        root_node.find("child"),
        Class::Runtime,
        "Node with name \"child\" does not exist in the current node \"root\".",
    );
    Ok(())
}

#[track_caller]
fn assert_node(expected_name: &str, expected_type: Builtin, actual_node: &NodeDefinition) {
    assert!(actual_node.is_instance_of(expected_type.class()));
    assert_eq!(
        actual_node.name().as_deref(),
        Some(expected_name.as_bytes())
    );
}

#[test]
fn testFindShouldHandleComplexConfigurationProperly() -> TestResult {
    let root_node = ArrayNodeDefinition::new("root");
    let mailer_node = ArrayNodeDefinition::new("mailer");
    mailer_node
        .children()
        .boolean_node("enable")?
        .end()
        .array_node("transports", None)?
        .end()
        .end();
    root_node
        .children()
        .array_node("social_media_channels", None)?
        .children()
        .boolean_node("enable")?
        .end()
        .array_node("twitter", None)?
        .end()
        .array_node("facebook", None)?
        .end()
        .array_node("instagram", None)?
        .children()
        .boolean_node("enable")?
        .end()
        .array_node("accounts", None)?
        .end()
        .end()
        .end()
        .end()
        .append(&mailer_node)
        .end();

    use Builtin::{Array, Boolean};
    assert_node(
        "social_media_channels",
        Array,
        &root_node.find("social_media_channels")?,
    );
    assert_node(
        "enable",
        Boolean,
        &root_node.find("social_media_channels.enable")?,
    );
    assert_node(
        "twitter",
        Array,
        &root_node.find("social_media_channels.twitter")?,
    );
    assert_node(
        "facebook",
        Array,
        &root_node.find("social_media_channels.facebook")?,
    );
    assert_node(
        "instagram",
        Array,
        &root_node.find("social_media_channels.instagram")?,
    );
    assert_node(
        "enable",
        Boolean,
        &root_node.find("social_media_channels.instagram.enable")?,
    );
    assert_node(
        "accounts",
        Array,
        &root_node.find("social_media_channels.instagram.accounts")?,
    );

    assert_node("enable", Boolean, &mailer_node.find("enable")?);
    assert_node("transports", Array, &mailer_node.find("transports")?);
    Ok(())
}

#[test]
fn testFindShouldWorkProperlyForNonDefaultPathSeparator() -> TestResult {
    let root_node = ArrayNodeDefinition::new("root");
    root_node
        .set_path_separator(".|")
        .children()
        .array_node("mailer.configuration", None)?
        .children()
        .boolean_node("enable")?
        .end()
        .array_node("transports", None)?
        .end()
        .end()
        .end();

    use Builtin::{Array, Boolean};
    assert_node(
        "mailer.configuration",
        Array,
        &root_node.find("mailer.configuration")?,
    );
    assert_node(
        "enable",
        Boolean,
        &root_node.find("mailer.configuration.|enable")?,
    );
    assert_node(
        "transports",
        Array,
        &root_node.find("mailer.configuration.|transports")?,
    );
    Ok(())
}
