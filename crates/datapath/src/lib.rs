// this readme is symlinked to the root of the repo,
// because cargo publish does odd things with paths.
// a relative path to the root readme will NOT work.
#![doc = include_str!("../README.md")]

// silence linter, used in README
#[cfg(test)]
use uuid as _;

mod datapath;
pub use datapath::*;

mod datapathfile;
pub use datapathfile::*;

mod schema;
pub use schema::*;

mod wildcardable;
pub use wildcardable::*;

pub use datapath_macro::datapath;
