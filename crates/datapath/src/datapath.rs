use std::{
	fmt::{Debug, Display},
	hash::Hash,
};

use crate::DatapathFile;

pub trait Datapath
where
	Self: Send + Sync + 'static,
	Self: Clone + Sized,
	Self: Eq + PartialEq + Hash,
	Self: Debug + Display,
{
	// Default

	/// Returns a [DatapathFile] with the given file at this datapath
	fn with_file(&self, file: impl Into<String>) -> DatapathFile<Self>;

	/// Parse a string as this datapath with a (possibly empty-string)
	/// file, returning `None` if this string is invalid.
	fn parse(path: &str) -> Option<DatapathFile<Self>>;
}
