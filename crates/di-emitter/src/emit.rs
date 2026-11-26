//! Writes the Rust container: placeholder types per PHP class, then
//! `Container` (application scope) and `RequestScope` (request scope)
//! with one accessor per service, in the shape of the converter fixture's
//! hand-written `container.rs`.
//!
//! The order of properties, calls and configurator is `PhpDumper`'s
//! (addServiceProperties, addServiceMethodCalls, addServiceConfigurator).
//!
//! Upstream: none

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use php_value::{Key, Value};
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;

use crate::{
    Error, Location,
    model::{ArgDef, Definition, FactoryDef, Graph, OnInvalid, Origin, StrPart},
    scope::{Analysis, Scope},
};

const CONTAINER_ID: &str = "service_container";

/// `HttpFoundation` -> `http_foundation`, `ORM` -> `orm`.
fn snake(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && i > 0 {
                let prev = chars[i - 1];
                let next_lower = chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
                if prev.is_ascii_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_ascii_uppercase() && next_lower)
                {
                    out.push('_');
                }
            }
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_owned();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("s_{out}")
    } else {
        out
    }
}

/// An identifier, raw if it is a keyword.
fn ident(s: &str) -> Ident {
    match s {
        "self" | "Self" | "super" | "crate" => Ident::new(&format!("{s}_"), Span::call_site()),
        _ if syn::parse_str::<Ident>(s).is_ok() => Ident::new(s, Span::call_site()),
        _ => Ident::new_raw(s, Span::call_site()),
    }
}

fn lit(s: &str) -> Literal {
    Literal::string(s)
}

#[derive(Default)]
struct Module {
    types: BTreeSet<String>,
    children: BTreeMap<String, Module>,
}

impl Module {
    fn tokens(&self) -> TokenStream {
        let types = self.types.iter().map(|t| {
            let t = ident(t);
            quote! { di_trial_runtime::placeholder!(#t); }
        });
        let children = self.children.iter().map(|(name, m)| {
            let name = ident(name);
            let body = m.tokens();
            quote! { pub mod #name { #body } }
        });
        quote! { #(#types)* #(#children)* }
    }
}

struct Names {
    /// PHP class -> path under `classes::`.
    class: HashMap<String, TokenStream>,
    /// Definition index -> accessor name.
    accessor: Vec<Ident>,
}

fn class_names(graph: &Graph) -> (Module, HashMap<String, TokenStream>) {
    let mut root = Module::default();
    let mut paths = HashMap::new();
    for d in &graph.services {
        if paths.contains_key(&d.class) {
            continue;
        }
        let segments: Vec<&str> = d.class.split('\\').filter(|s| !s.is_empty()).collect();
        let Some((last, namespace)) = segments.split_last() else {
            continue;
        };
        let mut m = &mut root;
        let mut path = quote! { classes };
        for ns in namespace {
            let name = snake(ns);
            let id = ident(&name);
            path = quote! { #path::#id };
            m = m.children.entry(name).or_default();
        }
        // A class whose snake name collides with a module of the same
        // namespace is impossible: classes keep their case.
        m.types.insert((*last).to_owned());
        let t = ident(last);
        paths.insert(d.class.clone(), quote! { #path::#t });
    }
    (root, paths)
}

const RESERVED: &[&str] = &["new", "get", "resolve", "parameter", "app", "this"];

fn accessor_names(graph: &Graph) -> Vec<Ident> {
    let mut used: HashSet<String> = RESERVED.iter().map(|s| (*s).to_owned()).collect();
    graph
        .services
        .iter()
        .map(|d| {
            let base = snake(&d.id);
            let mut name = base.clone();
            let mut n = 2;
            while !used.insert(name.clone()) {
                name = format!("{base}_{n}");
                n += 1;
            }
            ident(&name)
        })
        .collect()
}

fn key_tokens(key: Option<&Key>) -> TokenStream {
    match key {
        None => quote! { None },
        Some(Key::Int(i)) => quote! { Some(Key::Int(#i)) },
        Some(Key::Str(s)) => {
            let b = Literal::byte_string(s.as_bytes());
            quote! { Some(Key::Str(PhpString::from(&#b[..]))) }
        }
    }
}

fn value_tokens(v: &Value) -> Result<TokenStream, Error> {
    Ok(match v {
        Value::Null => quote! { Value::Null },
        Value::Bool(b) => quote! { Value::Bool(#b) },
        Value::Int(i) => quote! { Value::Int(#i) },
        Value::Float(f) => {
            let bits = f.to_bits();
            quote! { Value::Float(f64::from_bits(#bits)) }
        }
        Value::String(s) => {
            let b = Literal::byte_string(s.as_bytes());
            quote! { Value::String(PhpString::from(&#b[..])) }
        }
        Value::Array(a) => {
            let items = a
                .iter()
                .map(|(k, v)| {
                    let k = match k {
                        Key::Int(i) => quote! { Key::Int(#i) },
                        Key::Str(s) => {
                            let b = Literal::byte_string(s.as_bytes());
                            quote! { Key::Str(PhpString::from(&#b[..])) }
                        }
                    };
                    let v = value_tokens(v)?;
                    Ok(quote! { a.insert(#k, #v); })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            quote! { { let mut a = PhpArray::new(); #(#items)* Value::Array(a) } }
        }
        Value::Object(_) => {
            return Err(Error::new(
                Location::Unknown,
                "an object value cannot be compiled",
            ));
        }
    })
}

struct Emitter<'a> {
    graph: &'a Graph,
    an: &'a Analysis,
    names: Names,
}

impl Emitter<'_> {
    fn ty(&self, i: usize) -> &TokenStream {
        // Every definition's class was registered in class_names().
        &self.names.class[&self.graph.services[i].class]
    }

    fn cell(&self, i: usize) -> Ident {
        Ident::new(
            &format!("cell_{}", self.names.accessor[i]).replace("r#", ""),
            Span::call_site(),
        )
    }

    fn configure(&self, i: usize) -> Ident {
        Ident::new(
            &format!("configure_{}", self.names.accessor[i]).replace("r#", ""),
            Span::call_site(),
        )
    }

    fn if_built(&self, i: usize) -> Ident {
        Ident::new(
            &format!("{}_if_built", self.names.accessor[i]).replace("r#", ""),
            Span::call_site(),
        )
    }

    /// `self.x` or `self.app.x` for target `t` from code in scope `ctx`.
    fn owner(&self, t: usize, ctx: Scope, origin: &Origin) -> Result<TokenStream, Error> {
        match (ctx, self.an.scope[t]) {
            (a, b) if a == b => Ok(quote! { self }),
            (Scope::Request, Scope::App) => Ok(quote! { self.app }),
            _ => Err(Error::new(
                origin.location(),
                format!(
                    "internal: application-scoped code reaches request-scoped \"{}\"",
                    self.graph.services[t].id
                ),
            )),
        }
    }

    fn closure(
        &self,
        id: &str,
        on_invalid: OnInvalid,
        ctx: Scope,
        origin: &Origin,
    ) -> Result<TokenStream, Error> {
        let Some(t) = self.an.resolve(id) else {
            return Ok(quote! { { let f: ServiceClosure = Arc::new(|| None); f } });
        };
        let target = &self.graph.services[t];
        let via = if self.an.scope[t] == ctx {
            quote! { c }
        } else {
            self.owner(t, ctx, origin)?;
            quote! { c.app }
        };
        let body = if target.id == CONTAINER_ID {
            quote! { w.upgrade().map(|c| erase(c)) }
        } else if on_invalid == OnInvalid::IgnoreUninitialized {
            if target.shared {
                let ib = self.if_built(t);
                quote! { w.upgrade().and_then(|c| #via.#ib()).map(|s| erase(s)) }
            } else {
                quote! { None }
            }
        } else {
            let acc = &self.names.accessor[t];
            quote! { w.upgrade().map(|c| erase(#via.#acc())) }
        };
        Ok(
            quote! { { let w = self.this.clone(); let f: ServiceClosure = Arc::new(move || #body); f } },
        )
    }

    #[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
    fn arg(&self, a: &ArgDef, ctx: Scope) -> Result<Option<TokenStream>, Error> {
        Ok(Some(match a {
            ArgDef::Value(v) => {
                let v = value_tokens(v)?;
                quote! { Arg::Value(#v) }
            }
            ArgDef::Env(parts) => {
                if let [StrPart::Env(e)] = parts.as_slice() {
                    let e = lit(e);
                    quote! { Arg::Env(#e) }
                } else {
                    let parts = parts.iter().map(|p| match p {
                        StrPart::Text(t) => {
                            let b = Literal::byte_string(t.as_bytes());
                            quote! { Arg::Value(Value::String(PhpString::from(&#b[..]))) }
                        }
                        StrPart::Env(e) => {
                            let e = lit(e);
                            quote! { Arg::Env(#e) }
                        }
                    });
                    quote! { Arg::Concat(vec![#(#parts),*]) }
                }
            }
            ArgDef::Collection(items) => {
                let mut out = Vec::new();
                for (k, a) in items {
                    // A missing on-invalid="ignore" reference drops the
                    // element (ResolveInvalidReferencesPass).
                    if let Some(a) = self.arg(a, ctx)? {
                        let k = key_tokens(k.as_ref());
                        out.push(quote! { (#k, #a) });
                    }
                }
                quote! { Arg::Array(vec![#(#out),*]) }
            }
            ArgDef::Iterator(items) => {
                let mut out = Vec::new();
                for (k, a) in items {
                    let k = key_tokens(k.as_ref());
                    match a {
                        ArgDef::Ref {
                            id,
                            on_invalid,
                            origin,
                        } => {
                            // A missing reference drops the element.
                            if self.an.resolve(id).is_none() {
                                continue;
                            }
                            let c = self.closure(id, *on_invalid, ctx, origin)?;
                            out.push(quote! { (#k, Arg::Closure(#c)) });
                        }
                        ArgDef::Inline(_) | ArgDef::Closure { .. } | ArgDef::Iterator(_) => {
                            return Err(Error::new(
                                Location::Unknown,
                                "an inline service, closure or iterator inside an iterator is not supported",
                            ));
                        }
                        other => {
                            if let Some(a) = self.arg(other, ctx)? {
                                out.push(quote! { (#k, #a) });
                            }
                        }
                    }
                }
                quote! { Arg::Iterator(vec![#(#out),*]) }
            }
            ArgDef::Ref {
                id,
                on_invalid,
                origin,
            } => {
                let Some(t) = self.an.resolve(id) else {
                    return Ok(match on_invalid {
                        OnInvalid::Ignore => None,
                        _ => Some(quote! { Arg::Null }),
                    });
                };
                let target = &self.graph.services[t];
                if target.id == CONTAINER_ID {
                    quote! { self.this.upgrade().map_or(Arg::Null, |c| Arg::Service(erase(c))) }
                } else if *on_invalid == OnInvalid::IgnoreUninitialized {
                    if target.shared {
                        let owner = self.owner(t, ctx, origin)?;
                        let ib = self.if_built(t);
                        quote! { #owner.#ib().map_or(Arg::Null, |s| Arg::Service(erase(s))) }
                    } else {
                        quote! { Arg::Null }
                    }
                } else if target.lazy {
                    let c = self.closure(id, *on_invalid, ctx, origin)?;
                    quote! { Arg::Lazy(#c) }
                } else {
                    let owner = self.owner(t, ctx, origin)?;
                    let acc = &self.names.accessor[t];
                    quote! { Arg::Service(erase(#owner.#acc())) }
                }
            }
            ArgDef::Closure {
                id,
                on_invalid,
                origin,
            } => {
                let c = self.closure(id, *on_invalid, ctx, origin)?;
                quote! { Arg::Closure(#c) }
            }
            ArgDef::Inline(i) => {
                let e = self.inline(*i, ctx)?;
                quote! { Arg::Service(erase(#e)) }
            }
            ArgDef::Constant(c) => {
                let c = lit(c);
                quote! { Arg::Constant(#c) }
            }
        }))
    }

    fn args(&self, args: &[ArgDef], ctx: Scope) -> Result<TokenStream, Error> {
        let mut out = Vec::new();
        for a in args {
            if let Some(a) = self.arg(a, ctx)? {
                out.push(a);
            }
        }
        Ok(quote! { vec![#(#out),*] })
    }

    fn factory(&self, d: &Definition, ctx: Scope) -> Result<TokenStream, Error> {
        Ok(match &d.factory {
            None => quote! { Factory::Constructor },
            Some(FactoryDef::Function(name)) => {
                let n = lit(name);
                quote! { Factory::Function(#n) }
            }
            Some(FactoryDef::Static { class, method }) => {
                let (c, m) = (lit(class), lit(method));
                quote! { Factory::Static { class: #c, method: #m } }
            }
            Some(FactoryDef::Service { service, method }) => {
                let s = self
                    .arg(service, ctx)?
                    .unwrap_or_else(|| quote! { Arg::Null });
                let m = lit(method);
                quote! { Factory::Service { service: #s, method: #m } }
            }
        })
    }

    /// Statements that build `d` into `s` (without the cell check).
    fn construct(
        &self,
        i: usize,
        ctx: Scope,
        check: Option<&TokenStream>,
    ) -> Result<TokenStream, Error> {
        let d = &self.graph.services[i];
        let args = self.args(&d.arguments, ctx)?;
        let factory = self.factory(d, ctx)?;
        let (id, class) = (lit(&d.id), lit(&d.class));
        let ty = self.ty(i);
        Ok(quote! {
            let arguments: Vec<Arg> = #args;
            let factory = #factory;
            #check
            Arc::new(#ty(Instance::new(#id, #class, factory, arguments)))
        })
    }

    /// Properties, calls and configurator of `d`, run on `s`.
    fn after(&self, i: usize, ctx: Scope) -> Result<Option<TokenStream>, Error> {
        let d = &self.graph.services[i];
        let mut out = Vec::new();
        for (name, a) in &d.properties {
            if let Some(a) = self.arg(a, ctx)? {
                let name = lit(name);
                out.push(quote! { i.set_property(#name, #a); });
            }
        }
        for c in &d.calls {
            let args = self.args(&c.arguments, ctx)?;
            let m = lit(&c.method);
            if c.returns_clone {
                out.push(quote! { i.call_wither(#m, #args); });
            } else {
                out.push(quote! { i.call(#m, #args); });
            }
        }
        match &d.configurator {
            None => {}
            Some(FactoryDef::Function(name)) => {
                let n = lit(name);
                out.push(quote! { i.configure(Arg::Value(Value::string(#n)), "__invoke"); });
            }
            Some(FactoryDef::Static { class, method }) => {
                let (c, m) = (lit(class), lit(method));
                out.push(quote! { i.configure(Arg::Value(Value::string(#c)), #m); });
            }
            Some(FactoryDef::Service { service, method }) => {
                let s = self
                    .arg(service, ctx)?
                    .unwrap_or_else(|| quote! { Arg::Null });
                let m = lit(method);
                out.push(quote! { i.configure(#s, #m); });
            }
        }
        Ok((!out.is_empty()).then(|| quote! { let i = &s.0; #(#out)* }))
    }

    /// An inline definition, built where it is used.
    fn inline(&self, i: usize, ctx: Scope) -> Result<TokenStream, Error> {
        let build = self.construct(i, ctx, None)?;
        let after = self.after(i, ctx)?;
        Ok(quote! { { let s = { #build }; #after s } })
    }

    fn accessor(&self, i: usize) -> Result<(TokenStream, TokenStream, TokenStream), Error> {
        let d = &self.graph.services[i];
        let ctx = self.an.scope[i];
        let acc = &self.names.accessor[i];
        let ty = self.ty(i);
        let id = lit(&d.id);
        let doc = format!(" `{}` ({}).", d.id, d.class);
        if d.synthetic {
            if d.id == CONTAINER_ID {
                return Ok((
                    TokenStream::new(),
                    TokenStream::new(),
                    quote! {
                        #[doc = " `service_container`: the request scope itself."]
                        pub fn #acc(&self) -> Option<Arc<Self>> { self.this.upgrade() }
                    },
                ));
            }
            if ctx == Scope::Request {
                return Err(Error::new(
                    d.origin.location(),
                    format!("synthetic service \"{}\" is request-scoped", d.id),
                ));
            }
            let cell = self.cell(i);
            return Ok((
                quote! { #cell: Arc<#ty>, },
                quote! { #cell: #acc, },
                quote! {
                    #[doc = #doc]
                    pub fn #acc(&self) -> Arc<#ty> { self.#cell.clone() }
                },
            ));
        }
        let after = self.after(i, ctx)?;
        if !d.shared {
            let build = self.construct(i, ctx, None)?;
            return Ok((
                TokenStream::new(),
                TokenStream::new(),
                quote! {
                    #[doc = #doc]
                    #[doc = " Not shared: built at each use."]
                    pub fn #acc(&self) -> Arc<#ty> { let s = { #build }; #after s }
                },
            ));
        }
        let cell = self.cell(i);
        let ib = self.if_built(i);
        let check = quote! { if let Some(s) = self.#cell.get() { return s; } };
        let mut build = self.construct(i, ctx, Some(&check))?;
        // A wither returns a new object, so the service is only complete
        // after its calls: PhpDumper stores it after them, and so does
        // this (a cycle through such a service is not possible).
        let after = if d.calls.iter().any(|c| c.returns_clone) {
            build = quote! { let s = { #build }; #after s };
            None
        } else {
            after
        };
        let (cfg_field, cfg_init, run) = match &after {
            Some(after) => {
                let cfg = self.configure(i);
                (
                    quote! { #cfg: Configure, },
                    quote! { #cfg: Configure::default(), },
                    quote! { self.#cfg.run(|| { #after }); },
                )
            }
            None => Default::default(),
        };
        let _ = id;
        Ok((
            quote! { #cell: ServiceCell<#ty>, #cfg_field },
            quote! { #cell: ServiceCell::default(), #cfg_init },
            quote! {
                #[doc = #doc]
                pub fn #acc(&self) -> Arc<#ty> {
                    let s = self.#cell.get_or_build(|| { #build });
                    #run
                    s
                }

                pub fn #ib(&self) -> Option<Arc<#ty>> { self.#cell.get() }
            },
        ))
    }
}

#[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
pub fn emit(graph: &Graph, an: &Analysis) -> Result<String, Error> {
    let (classes, class) = class_names(graph);
    let names = Names {
        class,
        accessor: accessor_names(graph),
    };
    let e = Emitter { graph, an, names };

    let mut app = (Vec::new(), Vec::new(), Vec::new());
    let mut request = (Vec::new(), Vec::new(), Vec::new());
    let mut synthetic_params = Vec::new();
    for (i, d) in graph.services.iter().enumerate() {
        if d.inline {
            continue;
        }
        let (field, init, method) = e.accessor(i)?;
        let target = match an.scope[i] {
            Scope::App => &mut app,
            Scope::Request => &mut request,
        };
        target.0.push(field);
        target.1.push(init);
        target.2.push(method);
        if d.synthetic && d.id != CONTAINER_ID {
            let acc = &e.names.accessor[i];
            let ty = e.ty(i);
            synthetic_params.push(quote! { #acc: Arc<#ty> });
        }
    }

    // get(): public ids and public aliases; resolve(): every named id.
    let mut app_get = Vec::new();
    let mut app_resolve = Vec::new();
    let mut req_get = Vec::new();
    let mut req_resolve = Vec::new();
    let mut table = Vec::new();
    let call = |t: usize, ctx: Scope| -> TokenStream {
        let acc = &e.names.accessor[t];
        let d = &graph.services[t];
        if d.id == CONTAINER_ID {
            quote! { self.#acc().map(|c| erase(c)) }
        } else if ctx == an.scope[t] {
            quote! { Some(erase(self.#acc())) }
        } else {
            quote! { Some(erase(self.app.#acc())) }
        }
    };
    for (i, d) in graph.services.iter().enumerate() {
        if d.inline {
            continue;
        }
        let id = lit(&d.id);
        let class = lit(&d.class);
        let (req, public, shared) = (an.scope[i] == Scope::Request, d.public, d.shared);
        table.push(quote! { ServiceInfo { id: #id, class: #class, request_scoped: #req, public: #public, shared: #shared } });
        let rc = call(i, Scope::Request);
        req_resolve.push(quote! { #id => #rc, });
        if public {
            req_get.push(quote! { #id => #rc, });
        }
        if an.scope[i] == Scope::App {
            let ac = call(i, Scope::App);
            app_resolve.push(quote! { #id => #ac, });
            if public {
                app_get.push(quote! { #id => #ac, });
            }
        }
    }
    for (alias, t) in &an.public_aliases {
        let id = lit(alias);
        let rc = call(*t, Scope::Request);
        req_get.push(quote! { #id => #rc, });
        if an.scope[*t] == Scope::App {
            let ac = call(*t, Scope::App);
            app_get.push(quote! { #id => #ac, });
        }
    }

    let mut params = Vec::new();
    for (k, v) in &graph.parameters {
        let k = lit(k);
        let v = value_tokens(v)?;
        params.push(quote! { #k => Some(#v), });
    }

    let classes = classes.tokens();
    let (af, ai, am) = app;
    let (rf, ri, rm) = request;
    let tokens = quote! {
        use std::sync::{Arc, Weak};

        use di_trial_runtime::{
            Arg, Configure, Factory, Instance, Key, PhpString, Service, ServiceCell, ServiceClosure,
            ServiceObject, Value, erase,
        };
        use php_value::PhpArray;

        /// One placeholder type per PHP class of the container.
        pub mod classes { #classes }

        /// One row per named service.
        #[derive(Debug, Clone, Copy)]
        pub struct ServiceInfo {
            pub id: &'static str,
            pub class: &'static str,
            pub request_scoped: bool,
            pub public: bool,
            pub shared: bool,
        }

        pub const SERVICES: &[ServiceInfo] = &[#(#table),*];

        /// `$container->getParameter($name)` for the compiled parameters
        /// (`%env()%` ones are returned unresolved).
        pub fn parameter(name: &str) -> Option<Value> {
            match name {
                #(#params)*
                _ => None,
            }
        }

        /// Application-scoped services: built once, shared by all requests.
        pub struct Container {
            this: Weak<Container>,
            #(#af)*
        }

        impl Container {
            pub fn new(#(#synthetic_params),*) -> Arc<Self> {
                Arc::new_cyclic(|this| Self {
                    this: this.clone(),
                    #(#ai)*
                })
            }

            /// `$container->get($id)` for a public application-scoped
            /// service.
            pub fn get(&self, id: &str) -> Option<Service> {
                match id { #(#app_get)* _ => None }
            }

            /// Any application-scoped service, private ones included.
            pub fn resolve(&self, id: &str) -> Option<Service> {
                match id { #(#app_resolve)* _ => None }
            }

            #(#am)*
        }

        /// Request-scoped services: each request builds its own.
        pub struct RequestScope {
            this: Weak<RequestScope>,
            pub app: Arc<Container>,
            #(#rf)*
        }

        impl ServiceObject for RequestScope {
            fn instance(&self) -> Option<&Instance> { None }

            fn into_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> { self }
        }

        impl RequestScope {
            pub fn new(app: &Arc<Container>) -> Arc<Self> {
                Arc::new_cyclic(|this| Self {
                    this: this.clone(),
                    app: app.clone(),
                    #(#ri)*
                })
            }

            /// `$container->get($id)`: public services and aliases of both
            /// scopes.
            pub fn get(&self, id: &str) -> Option<Service> {
                match id { #(#req_get)* _ => None }
            }

            /// Any service, private ones included.
            pub fn resolve(&self, id: &str) -> Option<Service> {
                match id { #(#req_resolve)* _ => None }
            }

            #(#rm)*
        }
    };
    let file = syn::parse2::<syn::File>(tokens).map_err(|err| {
        Error::new(
            Location::Unknown,
            format!("internal: emitted code does not parse: {err}"),
        )
    })?;
    Ok(prettyplease::unparse(&file))
}

#[cfg(test)]
mod tests {
    use super::snake;

    #[test]
    fn snake_names() {
        assert_eq!(snake("HttpFoundation"), "http_foundation");
        assert_eq!(snake("ORM"), "orm");
        assert_eq!(snake("UXIconRuntime"), "ux_icon_runtime");
        assert_eq!(
            snake("doctrine.orm.default_entity_manager"),
            "doctrine_orm_default_entity_manager"
        );
        assert_eq!(snake("App\\Twig\\AppExtension"), "app_twig_app_extension");
        assert_eq!(
            snake(".service_locator.1vYpZ1B"),
            "service_locator_1v_yp_z1_b"
        );
    }
}
