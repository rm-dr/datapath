use std::{
	fmt::{Debug, Display},
	hash::Hash,
	str::FromStr,
};

use crate::Datapath;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DatapathFile<D: Datapath> {
	pub path: D,
	pub file: String,
}

impl<D: Datapath> FromStr for DatapathFile<D> {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Datapath::parse(s).ok_or(())
	}
}

impl<D: Datapath> Display for DatapathFile<D> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if self.file.is_empty() {
			write!(f, "{}", self.path)
		} else {
			write!(f, "{}/{}", self.path, self.file)
		}
	}
}
