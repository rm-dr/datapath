use std::sync::Arc;

//
// MARK: str
//

/// A reference to a substring of an [Arc<String>]
pub struct ArcSubstr<'a> {
	pub string: &'a Arc<String>,
	pub start: usize,
	pub end: usize,
}

impl<'a> ArcSubstr<'a> {
	pub fn as_str(&self) -> &str {
		&self.string[self.start..self.end]
	}

	pub fn from_string(string: &'a Arc<String>) -> Self {
		Self {
			start: 0,
			end: string.len(),
			string,
		}
	}
}

impl PartialEq for ArcSubstr<'_> {
	fn eq(&self, other: &Self) -> bool {
		self.as_str() == other.as_str()
	}
}

impl Eq for ArcSubstr<'_> {}

impl std::hash::Hash for ArcSubstr<'_> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.as_str().hash(state);
	}
}

impl PartialOrd for ArcSubstr<'_> {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for ArcSubstr<'_> {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.as_str().cmp(other.as_str())
	}
}

impl std::fmt::Debug for ArcSubstr<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.as_str().fmt(f)
	}
}

impl std::fmt::Display for ArcSubstr<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.as_str().fmt(f)
	}
}

//
// MARK: string
//

/// An owned [ArcSubstr]
pub struct ArcSubstring {
	pub string: Arc<String>,
	pub start: usize,
	pub end: usize,
}

impl ArcSubstring {
	pub fn as_str(&self) -> &str {
		&self.string[self.start..self.end]
	}

	pub fn from_string(string: Arc<String>) -> Self {
		Self {
			start: 0,
			end: string.len(),
			string,
		}
	}
}

impl PartialEq for ArcSubstring {
	fn eq(&self, other: &Self) -> bool {
		self.as_str() == other.as_str()
	}
}

impl Eq for ArcSubstring {}

impl std::hash::Hash for ArcSubstring {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.as_str().hash(state);
	}
}

impl PartialOrd for ArcSubstring {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for ArcSubstring {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.as_str().cmp(other.as_str())
	}
}

impl std::fmt::Debug for ArcSubstring {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.as_str().fmt(f)
	}
}

impl std::fmt::Display for ArcSubstring {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.as_str().fmt(f)
	}
}
