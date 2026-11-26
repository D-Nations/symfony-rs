# Symfony DependencyInjection

Private workspace crate: a port of `symfony/dependency-injection` v8.1.10
(`9b34cad8`), as installed by symfony/demo, for the build-time container
compiler of PLAN.md ("M3: DependencyInjection"). MIT; Symfony source and
tests copyright Fabien Potencier. Unofficial, not affiliated with or
endorsed by Symfony.

The crate grows from the November 24 trial (`spikes/di-trial`), which
stays as it is until this crate's compiler and Rust emitter replace it.

## Value model

- Values are php-value `Value`s, and arrays are `PhpArray`s with PHP's
  keys (a removed method call leaves a gap, as `unset()` does).
- `Definition` (with `ChildDefinition`), `Alias` and the argument objects
  are shared PHP objects (`object::Shared`): a cloned handle is the same
  object (`===`, `ptr_eq`), `clone_object()` is PHP's `clone`, and the
  handle is an object `Value`, so an inline definition can sit in an
  argument array. `Reference` (with `TypedReference`), `Parameter`,
  `Variable` and `EnvClosureArgument` are immutable objects.
- `ChildDefinition` and `TypedReference`/`TaggedIteratorArgument` are the
  base type with extra data; their class names and `instanceof` answers
  are upstream's.
- Identifiers (service ids, class names, parameter names, tag and method
  names) are UTF-8 `String`s.
- One `Error` type; its `ErrorKind` gives the exception class, and
  `instance_of()` answers for the component's classes, their SPL parents,
  `ExceptionInterface` and PSR-11's interfaces.

## Contract

| Contract | Source | Evidence |
|---|---|---|
| Exceptions: classes, hierarchy, messages (`ServiceNotFoundException` alternatives, `ParameterNotFoundException`'s rebuilt message and setters, circular-reference paths, `EnvParameterException`, `InvalidParameterTypeException` from a described parameter, `AutowiringFailedException`'s lazy message) | `Exception/*.php` | unit tests; the tests below |
| `ContainerInterface` invalid-reference constants | `ContainerInterface.php` (constants only) | DefinitionTest |
| `Reference`, `TypedReference` (name kept only when id = type), `Parameter`, `Variable` | `Reference.php`, `TypedReference.php`, `Parameter.php`, `Variable.php` | ReferenceTest (1), ParameterTest (1) |
| `Alias`: public, deprecation template checks and message | `Alias.php` | AliasTest (7) |
| `Definition`: every setter and getter, change tracking order, factory and configurator normalization, decorated service, argument bounds messages, method calls with PHP keys, tags, resource tags, bindings (key normalization and `BoundArgument` wrapping), errors (lazy ones rendered once), deprecation | `Definition.php` | DefinitionTest (33) |
| `ChildDefinition`: parent, `index_N` arguments, `$name` arguments | `ChildDefinition.php` | ChildDefinitionTest (11) |
| Arguments: `AbstractArgument`, `BoundArgument` (process-wide usage sequence), `EnvClosureArgument`, `IteratorArgument`, `TaggedIteratorArgument` (both constructor shapes, default method names, deprecations), `ServiceClosureArgument`, `ServiceLocatorArgument`; `ArgumentInterface` values | `Argument/*.php` (not the run-time classes) | TaggedIteratorArgumentTest (6) |
| `ParameterBag` and `FrozenParameterBag`: add/set (numeric names refused), get with alternatives and the nested-array hint, deprecations, non-empty parameters, resolve, `resolveValue()` (keys too), `resolveString()` (`%%`, whole-value placeholders, circular references), escape/unescape | `ParameterBag/ParameterBag.php`, `FrozenParameterBag.php`, `ParameterBagInterface.php` | ParameterBagTest (24), FrozenParameterBagTest (7) |
| `Container`: a compiled container as a `ContainerClass` factory table (method map, file map, aliases, synthetic and removed ids, privates), get/has/set/initialized/reset, circular-load detection, not-found alternatives, `camelize()`/`underscore()`, `getEnv()` through `container.env_var_processors_locator` | `Container.php` | ContainerTest (36; 1 pending on ContainerBuilder) |
| `ServiceLocator` (and `Argument\ServiceLocator`), with the caller class passed explicitly in place of `debug_backtrace()` | `ServiceLocator.php`, `Argument/ServiceLocator.php`, service-contracts `ServiceLocatorTrait.php` | ServiceLocatorTest (11) |
| `RewindableGenerator`, `ServicesResetter`, `ReverseContainer`, `TaggedContainerInterface`, `ContainerBag` | their files | RewindableGeneratorTest (3), ServicesResetterTest (8), ContainerBagTest (5) |
| `EnvVarProcessor`: every prefix of `getProvidedTypes()` (`require:` refused: it runs PHP), lookup order `$_ENV`, `$_SERVER` (not `HTTP_*`), `getenv()`, loaded vars, loaders, `env(NAME)` parameter; `StaticEnvVarLoader`, `EnvClosure`; the container's default processor | `EnvVarProcessor.php`, `EnvVarProcessorInterface.php`, `EnvVarLoaderInterface.php`, `StaticEnvVarLoader.php`, `Argument/EnvClosure.php` | EnvVarProcessorTest (49; `testRequireFile` n/a; `testGetEnvShuffle` pending on PHP's Mt19937), StaticEnvVarLoaderTest (1); 936 `getEnv()` calls and 52 lookups recorded from PHP 8.4.27 (`scripts/record.sh di-env-var-processor`) |
| ParameterBag vectors: 547 cases of `resolveValue()`, `resolveString()`, `resolve()` and `get()` recorded from PHP (`scripts/record.sh di-parameter-bag`); names keep their bytes | `ParameterBag/ParameterBag.php` | tests/parameter_bag_vectors.rs (2 non-UTF-8 `get()` names skipped) |
| `EnvPlaceholderParameterBag`: env placeholders (process-wide counter, as upstream's static one), defaults, provided types, merge; one `ParameterBag` type with an env flag | `ParameterBag/EnvPlaceholderParameterBag.php` | EnvPlaceholderParameterBagTest (17) |
| `ContainerBuilder` up to service creation: definitions, aliases, extensions and their config, resources (no Composer `inVendors()`: every resource is tracked), tags, `resolveEnvPlaceholders()`, `getEnv()` through an `EnvVarResolver`, `compile()`'s freeze; class lookups go through a `ClassRegistry` of class metadata. Creating a PHP object is refused, as is `@=` (no ExpressionLanguage) | `ContainerBuilder.php`, `Extension/ExtensionInterface.php` | ContainerBuilderTest (75; 67 pending, most on object creation) |
| Compiler skeleton: `Compiler`, `PassConfig` (upstream's 48 default passes in order; each unported pass refuses to run with its class name), `AbstractRecursivePass`, `ServiceReferenceGraph` (arena of nodes and edges) | `Compiler/Compiler.php`, `PassConfig.php`, `CompilerPassInterface.php`, `AbstractRecursivePass.php`, `ServiceReferenceGraph*.php` | PassConfigTest (2), AbstractRecursivePassTest (4) |
| Compiler passes, resolve group: `ResolveClassPass`, `ResolveChildDefinitionsPass`, `ResolveParameterPlaceHoldersPass`, `ResolveFactoryClassPass`, `ResolveNamedArgumentsPass`, `ResolveReferencesToAliasesPass`, `ResolveInvalidReferencesPass`, `ResolveDecoratorStackPass`, `ResolveEnvPlaceholdersPass`, `ResolveHotPathPass`, `ResolveNoPreloadPass`, `ResolveBindingsPass`, `ResolveInstanceofConditionalsPass`, `ResolveServiceSubscribersPass` (types from metadata strings in place of reflection) | `Compiler/Resolve*.php` | their upstream tests (123 of 123) |
| Compiler passes, check and remove group: `Analyze`/`InlineServiceDefinitions`/`RemovePrivateAliases`/`ReplaceAliasByActualDefinition`/`RemoveAbstractDefinitions`/`RemoveUnusedDefinitions`/`RemoveBuildParameters`, `Check*` (8, `CheckTypeDeclarationsPass` against the class registry), `DefinitionErrorExceptionPass`, `DecoratorServicePass`, `AliasDeprecatedPublicServicesPass`; no expression edges yet | their files | their upstream tests (164 of 182; the rest need object creation or `Expression`) |
| Compiler passes, register group: `MergeExtensionConfigurationPass`, `ExtensionCompilerPass`, `PriorityTaggedServiceTrait`, `ResolveTaggedIteratorArgumentPass`, `ServiceLocatorTagPass`, `RegisterEnvVarProcessorsPass`, `RegisterReverseContainerPass`, `RegisterServiceSubscribersPass`, `AutoAliasServicePass`, `ValidateEnvPlaceholdersPass`, `TagDecoratorPass`, `ResettableServicePass`, `AddBehaviorDescribingTagsPass`, `AutowireRequiredMethodsPass`, `AutowireRequiredPropertiesPass`, `AutowireAsDecoratorPass`, `RegisterAutoconfigureAttributesPass`, `AttributeAutoconfigurationPass`. Static methods (`getDefaultName()`, `getSubscribedServices()` ...) read their value from class metadata; attributes are metadata, and only the core attribute classes can be instantiated | their files | their upstream tests (132 of 153 outside AutowirePassTest; the rest need object creation, AutowirePass, or `Expression`) |
| `AutowirePass`, `ResolveAutowireInlineAttributesPass`: autowiring from class metadata (types, `#[Autowire]`, `#[Target]`, `#[AutowireCallable]`, `#[AutowireInline]`, lazy "autowiring failed" messages); `Expression` values and unknown `Autowire` subclasses are refused. Every default pass now runs; `compile()` of a container works up to service creation | `Compiler/AutowirePass.php`, `Compiler/ResolveAutowireInlineAttributesPass.php` | AutowirePassTest (84 of 93; the rest need object creation or `Expression`), ResolveAutowireInlineAttributesPassTest (2 of 3) |
| Dumpers: `XmlDumper`, `YamlDumper` (on symfony-yaml), `GraphvizDumper`; output byte-identical to PHP. `Expression` values are dumped by their string form | `Dumper/Dumper.php`, `DumperInterface.php`, `XmlDumper.php`, `YamlDumper.php`, `GraphvizDumper.php` | XmlDumperTest (18), YamlDumperTest (17; testDumpLoad needs YamlFileLoader), GraphvizDumperTest (5); 57 dumps recorded from PHP 8.4.27 (`scripts/record.sh di-dumpers`) |

Not yet ported: the loaders,
`PhpDumper` and `Preloader` (the Rust emitter replaces them),
the attributes and the kernel classes.
