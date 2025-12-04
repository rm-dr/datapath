use itertools::Itertools;
use std::{collections::HashMap, fmt::Display, str::FromStr};
use tracing::trace;
use trie_rs::map::{Trie, TrieBuilder};

mod rule;
pub use rule::Rule;

/// A path segment in an [`AnyDatapath`]
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum PathSegment {
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

//
// MARK: index
//

/// An in-memory cache of s3 paths.
#[derive(Debug)]
pub struct DatapathIndex {
	patterns: Trie<u8, Vec<String>>,
	len: usize,
}

impl DatapathIndex {
	/// Convert a query string to a trie search key by normalizing values to `*`.
	/// Stops at the first wildcard constant since it can't be used for prefix matching.
	fn query_to_key(query: &str) -> String {
		let trimmed = query.trim().trim_end_matches("**").trim_matches('/');
		let mut segments = Vec::new();
		for seg in trimmed.split('/') {
			let segment = match PathSegment::from_str(&seg) {
				Ok(x) => x,
				Err(_) => continue,
			};

			// lone stars and double-stars aren't in the trie
			if matches!(segment, PathSegment::Constant(ref s) if s == "*" || s == "**" ) {
				break;
			}

			segments.push(segment);
		}

		segments.iter_mut().for_each(|x| match x {
			PathSegment::Constant(_) => {}
			PathSegment::Value { value, .. } => *value = "*".into(),
		});

		segments.iter().join("/")
	}

	pub fn new_empty() -> Self {
		Self {
			patterns: TrieBuilder::new().build(),
			len: 0,
		}
	}

	pub fn new<S: Into<String>, I: Iterator<Item = S>>(paths: I) -> Self {
		let mut len = 0;
		let mut patterns = HashMap::new();

		for s in paths {
			let s: String = s.into();
			let mut segments = Vec::new();
			for seg in s.split('/') {
				segments.push(match PathSegment::from_str(&seg) {
					Ok(x) => x,
					Err(_) => continue,
				});
			}

			segments.iter_mut().for_each(|x| match x {
				PathSegment::Constant(_) => {}
				PathSegment::Value { value, .. } => *value = "*".into(),
			});

			let pattern = segments.iter().join("/");

			patterns.entry(pattern).or_insert(Vec::new()).push(s);
			len += 1;
		}

		let mut builder = TrieBuilder::new();
		for (k, v) in patterns {
			builder.push(k, v);
		}

		Self {
			len,
			patterns: builder.build(),
		}
	}

	#[cfg(feature = "tokio")]
	pub async fn async_new<S: Into<String>>(mut paths: tokio::sync::mpsc::Receiver<S>) -> Self {
		let mut len = 0;
		let mut patterns = HashMap::new();

		while let Some(s) = paths.recv().await {
			let s: String = s.into();
			let mut segments = Vec::new();
			for seg in s.split('/') {
				segments.push(match PathSegment::from_str(&seg) {
					Ok(x) => x,
					Err(_) => continue,
				});
			}

			segments.iter_mut().for_each(|x| match x {
				PathSegment::Constant(_) => {}
				PathSegment::Value { value, .. } => *value = "*".into(),
			});

			let pattern = segments.iter().join("/");

			patterns.entry(pattern).or_insert(Vec::new()).push(s);
			len += 1;
		}

		let mut builder = TrieBuilder::new();
		for (k, v) in patterns {
			builder.push(k, v);
		}

		Self {
			len,
			patterns: builder.build(),
		}
	}

	#[inline(always)]
	pub fn len(&self) -> usize {
		self.len
	}

	#[inline(always)]
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Given a datapath (that may contain wildcards) as a query,
	/// return all known datapaths that match it.
	///
	/// Returns an empty iterator if no paths match.
	/// Returns `None` if the query was invalid.
	pub fn query(&self, query: impl Into<String>) -> Option<impl Iterator<Item = String> + '_> {
		let query: String = query.into();
		let regex = rule::Rule::new(query.clone())?;
		let key = Self::query_to_key(&query);
		trace!("DatapathIndex key is {key}");

		Some(
			self.patterns
				.predictive_search::<String, _>(&key)
				.flat_map(|(_, strings)| strings.iter())
				.filter(move |s| regex.is_match(s))
				.cloned(),
		)
	}

	/// Like [Self::query], but with a precompiled rule
	pub fn query_rule<'a>(&'a self, rule: &'a rule::Rule) -> impl Iterator<Item = String> + 'a {
		let key = Self::query_to_key(rule.pattern());
		trace!("DatapathIndex key is {key}");

		self.patterns
			.predictive_search::<String, _>(&key)
			.flat_map(|(_, strings)| strings.iter())
			.filter(move |s| rule.is_match(s))
			.cloned()
	}

	/// Like [Self::query], but returns `true` if any paths match
	pub fn query_match(&self, query: impl Into<String>) -> Option<bool> {
		let query: String = query.into();
		let regex = rule::Rule::new(query.clone())?;
		let key = Self::query_to_key(&query);
		trace!("DatapathIndex key is {key}");

		for (_, strings) in self.patterns.predictive_search::<String, _>(&key) {
			for s in strings {
				if regex.is_match(s) {
					return Some(true);
				}
			}
		}

		return Some(false);
	}

	/// Like [Self::query_match], but with a precompiled rule
	pub fn query_rule_match<'a>(&'a self, rule: &'a rule::Rule) -> bool {
		let key = Self::query_to_key(&rule.pattern());
		trace!("DatapathIndex key is {key}");

		for (_, strings) in self.patterns.predictive_search::<String, _>(&key) {
			for s in strings {
				if rule.is_match(s) {
					return true;
				}
			}
		}

		return false;
	}
}

// MARK: index tests

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod index_tests {
	use super::*;

	#[test]
	fn datapath_index_empty() {
		let idx = DatapathIndex::new(std::iter::empty::<String>());
		let query = "web/domain=example.com";
		assert_eq!(idx.query(query).unwrap().count(), 0);
		assert!(idx.is_empty());
		assert_eq!(idx.len(), 0);
	}

	#[test]
	fn insert_and_lookup_exact_match() {
		let paths = vec!["web/domain=example.com/ts=1234"];
		let idx = DatapathIndex::new(paths.into_iter());

		// Exact match
		let results: Vec<_> = idx
			.query("web/domain=example.com/ts=1234")
			.unwrap()
			.collect();
		assert_eq!(results.len(), 1);
		assert_eq!(results[0], "web/domain=example.com/ts=1234");

		// No match
		let results: Vec<_> = idx.query("web/domain=other.com/ts=1234").unwrap().collect();
		assert_eq!(results.len(), 0);

		assert_eq!(idx.len(), 1);
	}

	#[test]
	fn wildcard_constant_match() {
		let paths = vec![
			"web/domain=example.com/ts=1234",
			"api/domain=example.com/ts=1234",
		];
		let idx = DatapathIndex::new(paths.into_iter());

		// Wildcard first segment
		let results: Vec<_> = idx.query("*/domain=example.com/ts=1234").unwrap().collect();
		assert_eq!(results.len(), 2);

		assert_eq!(idx.len(), 2);
	}

	#[test]
	fn wildcard_value_match() {
		let paths = vec![
			"web/domain=example.com/ts=1234",
			"web/domain=other.com/ts=1234",
		];
		let idx = DatapathIndex::new(paths.into_iter());

		// Wildcard domain
		let results: Vec<_> = idx.query("web/domain=*/ts=1234").unwrap().collect();
		assert_eq!(results.len(), 2);
	}

	#[test]
	fn multiple_datapaths() {
		let paths = vec![
			"web/domain=example.com/ts=1234",
			"web/domain=other.com/ts=1234",
			"api/domain=example.com/ts=5678",
		];
		let idx = DatapathIndex::new(paths.into_iter());

		// Specific lookup
		let results: Vec<_> = idx
			.query("web/domain=example.com/ts=1234")
			.unwrap()
			.collect();
		assert_eq!(results.len(), 1);
		assert_eq!(results[0], "web/domain=example.com/ts=1234");

		// Wildcard time lookup
		let results: Vec<_> = idx.query("web/domain=example.com/ts=*").unwrap().collect();
		assert_eq!(results.len(), 1);
		assert_eq!(results[0], "web/domain=example.com/ts=1234");

		// Double wildcard lookup
		let results: Vec<_> = idx.query("web/domain=*/ts=*").unwrap().collect();
		assert_eq!(results.len(), 2);

		assert_eq!(idx.len(), 3);
	}

	#[test]
	fn nested_wildcards() {
		let paths = vec![
			"web/domain=example.com/ts=1234/crawl/2.5",
			"web/domain=other.com/ts=5678/crawl/2.5",
			"web/domain=example.com/ts=9999/crawl/3.0",
		];
		let idx = DatapathIndex::new(paths.into_iter());

		// Multiple wildcards in path
		let results: Vec<_> = idx.query("web/domain=*/ts=*/crawl/*").unwrap().collect();
		assert_eq!(results.len(), 3);

		// Selective wildcards
		let results: Vec<_> = idx
			.query("web/domain=example.com/ts=*/crawl/*")
			.unwrap()
			.collect();
		assert_eq!(results.len(), 2);
	}

	#[test]
	fn partial_path_query() {
		let paths = vec!["web/domain=example.com/ts=1234/crawl/2.5"];
		let idx = DatapathIndex::new(paths.into_iter());

		// Query with fewer segments than the stored path
		let results: Vec<_> = idx.query("web/domain=example.com").unwrap().collect();
		assert_eq!(results.len(), 0);
	}

	#[test]
	fn longer_path_query() {
		let paths = vec!["web/domain=example.com"];
		let idx = DatapathIndex::new(paths.into_iter());

		// Query with more segments than the stored path
		let results: Vec<_> = idx
			.query("web/domain=example.com/ts=1234/crawl/2.5")
			.unwrap()
			.collect();
		assert_eq!(results.len(), 0);
	}

	#[test]
	fn query_match() {
		let paths = vec![
			"web/domain=example.com/ts=1234",
			"web/domain=other.com/ts=5678",
		];
		let idx = DatapathIndex::new(paths.into_iter());

		// Match exists
		assert_eq!(
			idx.query_match("web/domain=example.com/ts=1234").unwrap(),
			true
		);
		assert_eq!(idx.query_match("web/domain=*/ts=*").unwrap(), true);

		// No match
		assert_eq!(
			idx.query_match("api/domain=example.com/ts=1234").unwrap(),
			false
		);
		assert_eq!(
			idx.query_match("web/domain=missing.com/ts=9999").unwrap(),
			false
		);
	}

	#[test]
	fn suffix_wildcard() {
		let paths = vec![
			"web/domain=example.com/ts=1234/file1.json",
			"web/domain=example.com/ts=1234/file2.json",
			"web/domain=example.com/ts=5678/file3.json",
		];
		let idx = DatapathIndex::new(paths.into_iter());

		// Query with suffix wildcard
		let results: Vec<_> = idx.query("web/domain=example.com/**").unwrap().collect();
		assert_eq!(results.len(), 3);

		let results: Vec<_> = idx
			.query("web/domain=example.com/ts=1234/**")
			.unwrap()
			.collect();
		assert_eq!(results.len(), 2);
	}
}
