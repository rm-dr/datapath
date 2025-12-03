// silence linter, used in README
#[cfg(test)]
use uuid as _;

mod datapath;
pub use datapath::*;

mod datapathfile;
pub use datapathfile::*;

mod schema;
pub use schema::*;

pub use datapath_macro::datapath;
