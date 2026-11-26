//! Checks the graph and splits it into the two scopes of converter
//! contract 0.2, section 4.
//!
//! Upstream: none (the two messages are Symfony's:
//! ServiceCircularReferenceException.php and
//! CheckExceptionOnInvalidReferenceBehaviorPass.php)

use std::collections::{BTreeSet, HashMap};

use crate::{
    Config, Error, Report,
    model::{ArgDef, FactoryDef, Graph, OnInvalid, StrPart},
};

/// Where a service lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Built once and shared by all requests (`Send + Sync`).
    App,
    /// Built again for each request, as PHP-FPM builds a new container.
    Request,
}

/// How a definition needs another one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// Before it can be built (constructor, factory).
    Ctor,
    /// After it is stored (properties, method calls, configurator).
    After,
    /// Later, if at all (closures, iterators, lazy services).
    Lazy,
}

pub struct Analysis {
    /// Named definitions by id.
    pub index: HashMap<String, usize>,
    /// Alias id to the definition it finally names.
    pub aliases: HashMap<String, usize>,
    /// Public alias ids.
    pub public_aliases: Vec<(String, usize)>,
    pub scope: Vec<Scope>,
    pub report: Report,
}

impl Analysis {
    /// The definition an id names, through aliases.
    pub fn resolve(&self, id: &str) -> Option<usize> {
        self.index.get(id).or_else(|| self.aliases.get(id)).copied()
    }
}

fn edges_of(
    graph: &Graph,
    an: &Analysis,
    arg: &ArgDef,
    kind: Edge,
    out: &mut Vec<(usize, Edge)>,
    from: &str,
    report: &mut Report,
) -> Result<(), Error> {
    match arg {
        ArgDef::Value(_) | ArgDef::Constant(_) => {}
        ArgDef::Env(parts) => {
            for p in parts {
                if let StrPart::Env(e) = p {
                    report.env.push(e.clone());
                }
            }
        }
        ArgDef::Collection(items) => {
            for (_, a) in items {
                edges_of(graph, an, a, kind, out, from, report)?;
            }
        }
        ArgDef::Iterator(items) => {
            for (_, a) in items {
                edges_of(graph, an, a, Edge::Lazy, out, from, report)?;
            }
        }
        ArgDef::Ref {
            id,
            on_invalid,
            line,
        }
        | ArgDef::Closure {
            id,
            on_invalid,
            line,
        } => match an.resolve(id) {
            Some(target) => {
                let lazy = matches!(arg, ArgDef::Closure { .. })
                    || graph.services[target].lazy
                    || *on_invalid == OnInvalid::IgnoreUninitialized;
                out.push((target, if lazy { Edge::Lazy } else { kind }));
            }
            None if *on_invalid == OnInvalid::Exception => {
                return Err(Error {
                    line: *line,
                    message: format!(
                        "The service \"{from}\" has a dependency on a non-existent service \"{id}\"."
                    ),
                });
            }
            None => report.missing_optional.push(format!("{from} -> {id}")),
        },
        ArgDef::Inline(i) => out.push((*i, kind)),
    }
    Ok(())
}

fn factory_edges(
    graph: &Graph,
    an: &Analysis,
    f: &FactoryDef,
    kind: Edge,
    out: &mut Vec<(usize, Edge)>,
    from: &str,
    report: &mut Report,
) -> Result<(), Error> {
    if let FactoryDef::Service { service, .. } = f {
        edges_of(graph, an, service, kind, out, from, report)?;
    }
    Ok(())
}

#[expect(
    clippy::items_after_statements,
    clippy::too_many_lines,
    reason = "one analysis pass, with its depth-first searches nested"
)]
pub fn analyse(graph: &Graph, config: &Config) -> Result<Analysis, Error> {
    let mut an = Analysis {
        index: HashMap::new(),
        aliases: HashMap::new(),
        public_aliases: Vec::new(),
        scope: vec![Scope::App; graph.services.len()],
        report: Report::default(),
    };
    for (i, d) in graph.services.iter().enumerate() {
        if !d.inline {
            an.index.insert(d.id.clone(), i);
        }
    }
    for a in &graph.aliases {
        let mut target = a.target.as_str();
        let mut seen = BTreeSet::new();
        let resolved = loop {
            if let Some(&i) = an.index.get(target) {
                break i;
            }
            let next = graph.aliases.iter().find(|b| b.id == target);
            match next {
                Some(b) if seen.insert(b.id.clone()) => target = &b.target,
                _ => {
                    return Err(Error {
                        line: 0,
                        message: format!(
                            "The service \"{}\" has a dependency on a non-existent service \"{}\".",
                            a.id, a.target
                        ),
                    });
                }
            }
        };
        an.aliases.insert(a.id.clone(), resolved);
        if a.public {
            an.public_aliases.push((a.id.clone(), resolved));
        }
    }

    // Edges of every definition.
    let mut report = Report::default();
    let mut edges: Vec<Vec<(usize, Edge)>> = Vec::with_capacity(graph.services.len());
    for d in &graph.services {
        let mut out = Vec::new();
        for a in &d.arguments {
            edges_of(graph, &an, a, Edge::Ctor, &mut out, &d.id, &mut report)?;
        }
        if let Some(f) = &d.factory {
            factory_edges(graph, &an, f, Edge::Ctor, &mut out, &d.id, &mut report)?;
        }
        for (_, a) in &d.properties {
            edges_of(graph, &an, a, Edge::After, &mut out, &d.id, &mut report)?;
        }
        for c in &d.calls {
            for a in &c.arguments {
                edges_of(graph, &an, a, Edge::After, &mut out, &d.id, &mut report)?;
            }
        }
        if let Some(f) = &d.configurator {
            factory_edges(graph, &an, f, Edge::After, &mut out, &d.id, &mut report)?;
        }
        edges.push(out);
    }

    // Constructor cycles: PHP's CheckCircularReferencesPass refuses them,
    // and the Rust container would recurse without end.
    {
        let mut state = vec![0u8; graph.services.len()];
        let mut path = Vec::new();
        fn dfs(
            n: usize,
            edges: &[Vec<(usize, Edge)>],
            state: &mut [u8],
            path: &mut Vec<usize>,
            graph: &Graph,
        ) -> Result<(), Error> {
            state[n] = 1;
            path.push(n);
            for &(t, e) in &edges[n] {
                if e != Edge::Ctor {
                    continue;
                }
                if state[t] == 1 {
                    let start = path.iter().position(|&p| p == t).unwrap_or(0);
                    let mut ids: Vec<&str> = path[start..]
                        .iter()
                        .map(|&p| graph.services[p].id.as_str())
                        .collect();
                    ids.push(&graph.services[t].id);
                    return Err(Error {
                        line: graph.services[t].line,
                        message: format!(
                            "Circular reference detected for service \"{}\", path: \"{}\".",
                            graph.services[t].id,
                            ids.join(" -> ")
                        ),
                    });
                }
                if state[t] == 0 {
                    dfs(t, edges, state, path, graph)?;
                }
            }
            path.pop();
            state[n] = 2;
            Ok(())
        }
        for n in 0..graph.services.len() {
            if state[n] == 0 {
                dfs(n, &edges, &mut state, &mut path, graph)?;
            }
        }
    }

    // Cycles through calls (allowed): back edges over Ctor + After.
    {
        let mut state = vec![0u8; graph.services.len()];
        let mut path = Vec::new();
        let mut cycles = Vec::new();
        fn dfs(
            n: usize,
            edges: &[Vec<(usize, Edge)>],
            state: &mut [u8],
            path: &mut Vec<usize>,
            cycles: &mut Vec<Vec<usize>>,
        ) {
            state[n] = 1;
            path.push(n);
            for &(t, e) in &edges[n] {
                if e == Edge::Lazy {
                    continue;
                }
                if state[t] == 1 {
                    let start = path.iter().position(|&p| p == t).unwrap_or(0);
                    let mut c = path[start..].to_vec();
                    c.push(t);
                    cycles.push(c);
                } else if state[t] == 0 {
                    dfs(t, edges, state, path, cycles);
                }
            }
            path.pop();
            state[n] = 2;
        }
        for n in 0..graph.services.len() {
            if state[n] == 0 {
                dfs(n, &edges, &mut state, &mut path, &mut cycles);
            }
        }
        report.call_cycles = cycles
            .into_iter()
            .map(|c| {
                c.into_iter()
                    .map(|i| graph.services[i].id.clone())
                    .collect()
            })
            .collect();
    }

    // Scopes: seeds, then everything that reaches a request-scoped
    // service, by any kind of edge.
    let mut request = vec![false; graph.services.len()];
    for (i, d) in graph.services.iter().enumerate() {
        let seed = d.id == "service_container"
            || config.request_scoped_ids.contains(&d.id)
            || d.tags
                .iter()
                .any(|(t, _)| config.request_scoped_tags.contains(t));
        if seed {
            request[i] = true;
            report.request_seeds += 1;
        }
    }
    let mut reverse: Vec<Vec<usize>> = vec![Vec::new(); graph.services.len()];
    for (from, out) in edges.iter().enumerate() {
        for &(to, _) in out {
            reverse[to].push(from);
        }
    }
    let mut stack: Vec<usize> = (0..request.len()).filter(|&i| request[i]).collect();
    while let Some(n) = stack.pop() {
        for &from in &reverse[n] {
            if !request[from] {
                request[from] = true;
                stack.push(from);
            }
        }
    }
    // An inline definition is built inside its owner, so it is emitted in
    // the owner's scope; mark owners of request-scoped inline ones (done
    // by the propagation above, since owner -> inline is an edge).
    for (i, r) in request.iter().enumerate() {
        an.scope[i] = if *r { Scope::Request } else { Scope::App };
    }

    report.services = graph.services.iter().filter(|d| !d.inline).count();
    report.inline = graph.services.len() - report.services;
    report.aliases = graph.aliases.len();
    report.classes = graph
        .services
        .iter()
        .map(|d| d.class.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    report.app_scoped = graph
        .services
        .iter()
        .enumerate()
        .filter(|(i, d)| !d.inline && an.scope[*i] == Scope::App)
        .count();
    report.request_scoped = report.services - report.app_scoped;
    report.env.sort();
    report.env.dedup();
    an.report = report;
    Ok(an)
}
