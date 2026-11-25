//! Port of Doctrine\DBAL\Platforms\AbstractPlatform.php, one group of
//! methods at a time.
//!
//! The PHP class is abstract: its abstract methods are required trait
//! methods, its concrete methods are default methods with upstream's body.
//! A platform (`SQLitePlatform`, ...) implements the trait and overrides
//! what its PHP class overrides. Tests stand in for PHPUnit's
//! `createMock(AbstractPlatform::class)` with their own implementation.
//!
//! Methods are added with the slice that first needs them, each group under
//! a comment that names the slice (Query, Types, ...), so slices merge
//! without reordering one another.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/AbstractPlatform.php

/// Base contract for the SQL dialects (`AbstractPlatform`).
pub trait AbstractPlatform {}
