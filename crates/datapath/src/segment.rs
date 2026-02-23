use itertools::Itertools;
use std::{fmt::Display, str::FromStr};

/// A path segment in an [`AnyDatapath`]
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum PathSegment {
	/// A constant value, like `web`
	Constant(String),

	/// A key=value partition, like `domain=gouletpens.com`
	Value { key: String, value: String },
}

impl Display for PathSegment {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			PathSegment::Constant(x) => write!(f, "{x}"),
			PathSegment::Value { key, value } => write!(f, "{key}={value}"),
		}
	}
}

impl FromStr for PathSegment {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if s.contains("\n") {
			return Err(());
		}

		if s.is_empty() {
			return Err(());
		}

		return Ok(if s.contains("=") {
			let mut s = s.split("=");
			let key = s.next().ok_or(())?.to_owned();
			let value = s.join("=");
			Self::Value { key, value }
		} else {
			Self::Constant(s.to_owned())
		});
	}
}
