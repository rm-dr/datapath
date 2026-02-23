use itertools::Itertools;
use smartstring::{LazyCompact, SmartString};
use std::{collections::HashMap, sync::Arc};

use crate::Rule;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SchemaSegments {
	/// A const path segment like `web`
	Const(SmartString<LazyCompact>),

	/// A prefix path segment like `domain=`,
	/// without the equals sign
	Prefix(SmartString<LazyCompact>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Schema {
	segments: Vec<SchemaSegments>,
}

impl Schema {
	pub fn from_path(path: &str, max_len: usize) -> Self {
		let segments = path
			.split('/')
			.take(max_len)
			.map(|x| {
				if x == "*" || x == "**" {
					return None;
				}

				let is_prefix = x.contains('=');
				Some(match is_prefix {
					true => SchemaSegments::Prefix(
						x.split('=')
							.next()
							.expect("split must return at least one item")
							.into(),
					),
					false => SchemaSegments::Const(x.into()),
				})
			})
			.while_some()
			.collect::<Vec<_>>();

		Self { segments }
	}

	pub fn exemplar(&self) -> String {
		self.segments
			.iter()
			.map(|x| match x {
				SchemaSegments::Const(x) => x.to_string(),
				SchemaSegments::Prefix(x) => format!("{x}=*"),
			})
			.join("/")
	}
}

/// An in-memory cache of s3 paths.
#[derive(Debug)]
pub struct DatapathVecIndex {
	/// Array of (schema, paths with that schema)
	/// - all paths belong to exactly one schema
	/// - we use the fact that order in both vecs is constant
	paths: Vec<(Schema, Vec<Arc<String>>)>,

	len: usize,
	schema_len: usize,
}

impl DatapathVecIndex {
	pub fn new_empty(schema_len: usize) -> Self {
		Self {
			paths: Vec::new(),
			len: 0,
			schema_len,
		}
	}

	pub fn schema_len(&self) -> usize {
		self.schema_len
	}

	pub fn new<S: Into<String>, I: Iterator<Item = S>>(schema_len: usize, paths: I) -> Self {
		let mut len = 0;
		let mut map: HashMap<Schema, Vec<Arc<String>>> = HashMap::new();
		for p in paths {
			let p = Arc::new(p.into());
			let schema = Schema::from_path(&p, schema_len);
			map.entry(schema).or_default().push(p);
			len += 1;
		}

		Self {
			schema_len,
			len,
			paths: map.into_iter().collect(),
		}
	}

	#[cfg(feature = "tokio")]
	pub async fn async_new<S: Into<String>>(
		schema_len: usize,
		mut stream: tokio::sync::mpsc::Receiver<S>,
	) -> Self {
		let mut len = 0;
		let mut map: HashMap<Schema, Vec<Arc<String>>> = HashMap::new();
		while let Some(p) = stream.recv().await {
			let p = Arc::new(p.into());
			let schema = Schema::from_path(&p, schema_len);
			map.entry(schema).or_default().push(p);
			len += 1;
		}

		Self {
			schema_len,
			len,
			paths: map.into_iter().collect(),
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
		let regex = Rule::new(query.clone())?;

		// This is not a bug, we want all segments from query.
		let query_schema = Schema::from_path(&query, query.len()).exemplar();

		Some(
			self.paths
				.iter()
				.filter(move |(schema, _)| {
					let schema = schema.exemplar();
					schema.starts_with(&query_schema) || query_schema.starts_with(&schema)
				})
				.flat_map(|(_, paths)| paths.iter())
				.filter(move |path| regex.is_match(path.as_str()))
				.map(|arc_str| arc_str.as_ref().clone()),
		)
	}

	/// Like [Self::query], but with a precompiled rule
	pub fn query_rule<'a>(&'a self, rule: &'a Rule) -> impl Iterator<Item = String> + 'a {
		let query = rule.pattern();
		let query_schema = Schema::from_path(&query, query.len()).exemplar();

		self.paths
			.iter()
			.filter(move |(schema, _)| {
				let schema = schema.exemplar();
				schema.starts_with(&query_schema) || query_schema.starts_with(&schema)
			})
			.flat_map(|(_, paths)| paths.iter())
			.filter(move |path| rule.is_match(path.as_str()))
			.map(|arc_str| arc_str.as_ref().clone())
	}

	/// Like [Self::query], but returns `true` if any paths match
	pub fn query_match(&self, query: impl Into<String>) -> Option<bool> {
		let query: String = query.into();
		let regex = Rule::new(query.clone())?;
		let query_schema = Schema::from_path(&query, query.len()).exemplar();

		Some(
			self.paths
				.iter()
				.filter(move |(schema, _)| {
					let schema = schema.exemplar();
					schema.starts_with(&query_schema) || query_schema.starts_with(&schema)
				})
				.flat_map(|(_, paths)| paths.iter())
				.any(|path| regex.is_match(path.as_str())),
		)
	}

	/// Like [Self::query_match], but with a precompiled rule
	pub fn query_rule_match<'a>(&'a self, rule: &'a Rule) -> bool {
		let query = rule.pattern();
		let query_schema = Schema::from_path(&query, query.len()).exemplar();

		self.paths
			.iter()
			.filter(move |(schema, _)| {
				let schema = schema.exemplar();
				schema.starts_with(&query_schema) || query_schema.starts_with(&schema)
			})
			.flat_map(|(_, paths)| paths.iter())
			.any(|path| rule.is_match(path.as_str()))
	}
}

// MARK: index tests

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod index_tests {
	use super::*;

	#[test]
	fn datapath_index_empty() {
		let idx = DatapathVecIndex::new(3, std::iter::empty::<String>());
		let query = "web/domain=example.com";
		assert_eq!(idx.query(query).unwrap().count(), 0);
		assert!(idx.is_empty());
		assert_eq!(idx.len(), 0);
	}

	#[test]
	fn insert_and_lookup_exact_match() {
		let paths = vec!["web/domain=example.com/ts=1234"];
		let idx = DatapathVecIndex::new(3, paths.into_iter());

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
		let idx = DatapathVecIndex::new(3, paths.into_iter());

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
		let idx = DatapathVecIndex::new(3, paths.into_iter());

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
		let idx = DatapathVecIndex::new(3, paths.into_iter());

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
		let idx = DatapathVecIndex::new(3, paths.into_iter());

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
		let idx = DatapathVecIndex::new(3, paths.into_iter());

		// Query with fewer segments than the stored path
		let results: Vec<_> = idx.query("web/domain=example.com").unwrap().collect();
		assert_eq!(results.len(), 0);
	}

	#[test]
	fn longer_path_query() {
		let paths = vec!["web/domain=example.com"];
		let idx = DatapathVecIndex::new(3, paths.into_iter());

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
		let idx = DatapathVecIndex::new(3, paths.into_iter());

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
		let idx = DatapathVecIndex::new(3, paths.into_iter());

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
