use std::{
	fmt::{Debug, Display},
	hash::Hash,
	str::FromStr,
};

/// A wrapper for wildcardable partition values.
/// Allows us to specify, for example, `ts=1337` and `ts=*`.
#[derive(Debug, PartialEq, Eq, Hash, Default)]
pub enum Wildcardable<T: FromStr + Display + Debug + Eq + PartialEq + Hash> {
	/// This value is wildcarded with a star,
	/// as in `ts=*`
	#[default]
	Star,

	/// This value is explicitly given,
	/// as in `ts=1337`
	Value(T),
}

impl<T: FromStr + Display + Debug + Eq + PartialEq + Hash> Wildcardable<T> {
	pub fn inner(&self) -> Option<&T> {
		match self {
			Self::Star => None,
			Self::Value(x) => Some(x),
		}
	}

	pub fn into_inner(self) -> Option<T> {
		match self {
			Self::Star => None,
			Self::Value(x) => Some(x),
		}
	}
}

impl<T: FromStr + Display + Debug + Eq + PartialEq + Hash + Copy> Copy for Wildcardable<T> {}

impl<T: FromStr + Display + Debug + Eq + PartialEq + Hash + Clone> Clone for Wildcardable<T> {
	fn clone(&self) -> Self {
		match self {
			Self::Star => Self::Star,
			Self::Value(x) => Self::Value(x.clone()),
		}
	}
}

impl<T: FromStr + Display + Debug + Eq + PartialEq + Hash> Display for Wildcardable<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Star => write!(f, "*"),
			Self::Value(x) => write!(f, "{x}"),
		}
	}
}

impl<T: FromStr + Display + Debug + Eq + PartialEq + Hash> FromStr for Wildcardable<T> {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		return Ok(match s {
			"*" => Self::Star,
			value => Self::Value(value.parse().map_err(|_err| ())?),
		});
	}
}

impl<T: FromStr + Display + Debug + Eq + PartialEq + Hash> From<T> for Wildcardable<T> {
	fn from(value: T) -> Self {
		Self::Value(value)
	}
}

impl<T: FromStr + Display + Debug + Eq + PartialEq + Hash> From<Wildcardable<T>> for Option<T> {
	fn from(value: Wildcardable<T>) -> Self {
		value.into_inner()
	}
}
