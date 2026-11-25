//! Port of Symfony\Component\Config\Definition\Builder\TreeBuilder.php:
//! the entry point of a configuration tree. It creates the root
//! definition and builds (once) the tree of nodes.
//!
//! `TreeBuilder<C>` names the root's class as upstream's `@template T`
//! does (`'array'` by default); a root of another class than `C` (a type
//! mapped to another class by a custom `NodeBuilder`) is refused when the
//! `TreeBuilder` is created, where PHP would fail at the first call the
//! root does not have.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/TreeBuilder.php

use std::{
    fmt,
    marker::PhantomData,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use crate::definition::{
    base_node::Node,
    builder::{
        node_builder::NodeBuilder,
        node_definition::{Def, Definition, IntoName, Parent, ParentHandle, class},
    },
    exception::{Class, Error},
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\Builder\\TreeBuilder";

struct TreeState {
    tree: Option<Node>,
    root: Def,
}

/// A `TreeBuilder` whose root definition is of class `C`.
pub struct TreeBuilder<C = class::Array> {
    inner: Arc<Mutex<TreeState>>,
    class: PhantomData<fn() -> C>,
}

impl<C> Clone for TreeBuilder<C> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            class: PhantomData,
        }
    }
}

impl<C> fmt::Debug for TreeBuilder<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TreeBuilder")
    }
}

impl<C> ParentHandle for TreeBuilder<C> {
    fn as_parent(&self) -> Parent {
        Parent::Object(CLASS.to_owned())
    }
}

impl TreeBuilder<class::Array> {
    /// `new TreeBuilder($name)`: an array root.
    pub fn new(name: impl IntoName) -> Result<Self, Error> {
        Self::with_type(name, "array", None)
    }

    /// `new TreeBuilder($name, 'array', $builder)`.
    pub fn with_builder(name: impl IntoName, builder: NodeBuilder) -> Result<Self, Error> {
        Self::with_type(name, "array", Some(builder))
    }
}

impl<C: class::Marker> TreeBuilder<C> {
    fn lock(&self) -> MutexGuard<'_, TreeState> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `new TreeBuilder($name, $type, $builder)`.
    pub fn with_type(
        name: impl IntoName,
        type_name: &str,
        builder: Option<NodeBuilder>,
    ) -> Result<Self, Error> {
        let builder = builder.unwrap_or_default();
        let root = builder.node(name, type_name)?;
        if !<C as class::Marker>::accepts(root.def.builtin()) {
            return Err(Error::new(
                Class::TypeError,
                format!(
                    "{CLASS}::getRootNode(): Return value must be of type {}, {} returned",
                    C::CLASS,
                    root.class()
                ),
            ));
        }
        root.def.set_parent(&Parent::Object(CLASS.to_owned()));
        Ok(Self {
            inner: Arc::new(Mutex::new(TreeState {
                tree: None,
                root: root.def,
            })),
            class: PhantomData,
        })
    }

    /// `getRootNode()`.
    pub fn get_root_node(&self) -> Definition<C, Self> {
        Definition::from_def(self.lock().root.clone(), self.clone())
    }

    /// `buildTree()`: the tree, built once.
    pub fn build_tree(&self) -> Result<Node, Error> {
        let (tree, root) = {
            let s = self.lock();
            (s.tree.clone(), s.root.clone())
        };
        if let Some(tree) = tree {
            return Ok(tree);
        }
        let tree = root.get_node(true)?;
        self.lock().tree = Some(tree.clone());
        Ok(tree)
    }

    /// `setPathSeparator($separator)`: forgets the built tree.
    pub fn set_path_separator(&self, separator: impl AsRef<[u8]>) {
        let root = {
            let mut s = self.lock();
            s.tree = None;
            s.root.clone()
        };
        root.set_path_separator(separator.as_ref());
    }
}
