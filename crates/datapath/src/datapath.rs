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
	/// The exact pattern string passed to the macro that generated this struct
	const PATTERN: &'static str;

	/// A tuple of this path's parameter types, in the order they appear in the pattern
	type Tuple;

	/// [Datapath::Tuple], but each type is wrapped in a [crate::Wildcardable].
	type WildcardableTuple;

	fn from_tuple(tuple: Self::Tuple) -> Self;
	fn to_tuple(self) -> Self::Tuple;

	/// Return a string where wildcarded partitions are `*`.
	fn from_wildcardable(tuple: Self::WildcardableTuple) -> String;

	/// Returns a [DatapathFile] with the given file at this datapath
	fn with_file(&self, file: impl Into<String>) -> DatapathFile<Self>;

	/// Parse a string as this datapath with a (possibly empty-string)
	/// file, returning `None` if this string is invalid.
	fn parse(path: &str) -> Option<DatapathFile<Self>>;

	/// Get the string value of the field with the given name,
	/// if it exists.
	fn field(&self, name: &str) -> Option<String>;
}
