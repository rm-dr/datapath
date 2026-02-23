use regex::Regex;
use tracing::warn;

//
// MARK: rule
//

#[derive(Debug)]
enum RegexSegment {
	/// A single segment
	Single(String),

	/// An optional doublestar segment
	DoubleStar,
}

impl RegexSegment {
	/// Returns the regex pattern of this part,
	/// prefixed with a /.
	fn to_regex_part(&self, prev: Option<&Self>, next: Option<&Self>) -> String {
		match (prev, self, next) {
			// Consecutive single segments need a trailing slash
			(_, Self::Single(x), Some(Self::Single(_))) => format!("{x}[/]"),

			// Terminal single segments don't need a trailing slash
			(_, Self::Single(x), None) => x.to_owned(),

			// Neighboring doublestar is always responsible for slashes
			(_, Self::Single(x), Some(Self::DoubleStar)) => x.to_owned(),

			// No additional slashes
			(None, Self::DoubleStar, None) => "((?:.*)?)".into(),

			// Leading slash
			(Some(Self::Single(_)), Self::DoubleStar, None) => "((?:[/].*)?)".into(),

			// Trailing slash
			(None, Self::DoubleStar, Some(Self::Single(_))) => "((?:.*[/])?)".into(),

			// Leading and trailing slash.
			// Also, replace self with a [/] when empty.
			(Some(Self::Single(_)), Self::DoubleStar, Some(Self::Single(_))) => {
				"((?:[/].*[/])|[/])".into()
			}

			// Doublestars cannot be neighbors
			(_, Self::DoubleStar, Some(Self::DoubleStar))
			| (Some(Self::DoubleStar), Self::DoubleStar, _) => {
				unreachable!("consecutive doublestars must be reduced")
			}
		}
	}
}

#[derive(Debug, Clone)]
pub struct Rule {
	regex: Regex,
	pattern: String,
}

impl Rule {
	pub fn pattern(&self) -> &str {
		&self.pattern
	}

	pub fn regex(&self) -> &Regex {
		&self.regex
	}

	pub fn is_match(&self, s: &str) -> bool {
		self.regex.is_match(s)
	}

	pub fn raw_regex_str(&self) -> String {
		Self::regex_str(self.pattern()).unwrap()
	}

	fn regex_str(pattern: &str) -> Option<String> {
		// Split on slashes or stars
		// This is a lot like .split("/"), but handles
		// the edge case where ** is not delimited by slashes
		// (`root**test` is equivalent to `root/**/test`)
		let segments = {
			#[expect(clippy::unwrap_used)]
			let re = Regex::new("[*]{2,}|[/]").unwrap();
			let split = re.find_iter(&pattern);

			let bounds = split
				.into_iter()
				.flat_map(|x| {
					let r = x.range();
					let a = r.start;
					let b = r.end;
					[a, b]
				})
				.chain([pattern.len()])
				.collect::<Vec<_>>();

			let mut parts = Vec::new();
			let mut last = 0;
			for next in bounds {
				let seg = &pattern[last..next];
				// Consecutive slashes are identical to a single slash
				if seg != "/" && !seg.is_empty() {
					parts.push(seg);
				}
				last = next;
			}

			parts
		};

		let mut rebuilt_segments = Vec::new();
		let mut last_was_doublestar = false;
		for segment in segments {
			// This is a wildcard regex
			// (**, ***, etc)
			if segment.len() > 1 && segment.chars().all(|x| x == '*') {
				match segment {
					"**" => {
						// Consecutive doublestars are meaningless
						if !last_was_doublestar {
							rebuilt_segments.push(RegexSegment::DoubleStar);
						}
						last_was_doublestar = true;
					}
					_ => return None,
				}
				continue;
			}
			last_was_doublestar = false;

			let parts = segment.split("*").collect::<Vec<_>>();

			let mut rebuilt = String::new();
			for (i, part) in parts.into_iter().enumerate() {
				if i != 0 {
					rebuilt.push_str("([^/]*)")
				}

				rebuilt.push_str(&regex::escape(part));
			}

			rebuilt_segments.push(RegexSegment::Single(rebuilt));
		}

		let mut re_built = String::new();
		let mut prev = None;
		for (i, seg) in rebuilt_segments.iter().enumerate() {
			let next = rebuilt_segments.get(i + 1);
			re_built.push_str(&seg.to_regex_part(prev, next));
			prev = Some(seg);
		}

		return Some(re_built);
	}

	/// Returns `None` if this rule was invalid.
	pub fn new(pattern: impl Into<String>) -> Option<Self> {
		let pattern: String = pattern.into();

		if pattern.ends_with("/") {
			warn!("Pattern `{pattern}` has a trailing slash which will be ignored")
		}

		if pattern.starts_with("/") {
			warn!("Pattern `{pattern}` has a leading slash which will be ignored")
		}

		let re_built = Self::regex_str(&pattern)?;
		let re_built = format!("^{re_built}$");

		// This regex should always be valid
		#[expect(clippy::unwrap_used)]
		let regex = Regex::new(&re_built).unwrap();

		Some(Self { regex, pattern })
	}
}

//
// MARK: tests
//

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod rule_tests {
	use super::*;

	#[test]
	fn simple() {
		let regex = Rule::new("file.txt").unwrap();

		assert!(regex.is_match("file.txt"));
		assert!(!regex.is_match("other.txt"));
		assert!(!regex.is_match("path/file.txt"));
	}

	#[test]
	fn simple_dir() {
		let regex = Rule::new("dir/file.txt").unwrap();

		assert!(regex.is_match("dir/file.txt"));
		assert!(!regex.is_match("file.txt"));
		assert!(!regex.is_match("other/file.txt"));
	}

	#[test]
	fn simple_star() {
		let regex = Rule::new("*.txt").unwrap();

		assert!(regex.is_match("file.txt"));
		assert!(regex.is_match("other.txt"));
		assert!(!regex.is_match("file.jpg"));
		assert!(!regex.is_match("nested/file.txt"));
	}

	#[test]
	fn simple_doublestar() {
		let regex = Rule::new("**/*.txt").unwrap();

		assert!(regex.is_match("file.txt"));
		assert!(regex.is_match("dir/file.txt"));
		assert!(regex.is_match("dir/subdir/file.txt"));
		assert!(!regex.is_match("file.jpg"));
		assert!(!regex.is_match("dir/file.jpg"));
	}

	#[test]
	fn consecutive_doublestar() {
		let regex = Rule::new("**/**/**/*.txt").unwrap();

		assert!(regex.is_match("file.txt"));
		assert!(regex.is_match("dir/file.txt"));
		assert!(regex.is_match("dir/subdir/file.txt"));
		assert!(!regex.is_match("file.jpg"));
		assert!(!regex.is_match("dir/file.jpg"));
	}

	#[test]
	fn dual_star() {
		let regex = Rule::new("**/*a*").unwrap();

		assert!(regex.is_match("fileafile"));
		assert!(regex.is_match("dir/fileafile"));
		assert!(regex.is_match("filea"));
		assert!(regex.is_match("dir/filea"));
		assert!(regex.is_match("afile"));
		assert!(regex.is_match("dir/afile"));
		assert!(!regex.is_match("noletter"));
		assert!(!regex.is_match("dir/noletter"));
	}

	#[test]
	fn single_end() {
		let regex = Rule::new("**/*").unwrap();

		assert!(regex.is_match("file"));
		assert!(regex.is_match("dir/file"));
		assert!(regex.is_match("a/b/c/dir/file"));
	}

	#[test]
	fn doublestar_end() {
		let regex = Rule::new("root/**").unwrap();

		assert!(regex.is_match("root/file"));
		assert!(!regex.is_match("dir/file"));
	}

	#[test]
	fn doublestar_start() {
		let regex = Rule::new("**/dir").unwrap();

		assert!(regex.is_match("dir"));
		assert!(regex.is_match("a/b/dir"));
		assert!(!regex.is_match("dir/file"));
	}

	#[test]
	fn doublestar_adjacent_before() {
		let regex = Rule::new("root/**test").unwrap();

		assert!(regex.is_match("root/test"));
		assert!(regex.is_match("root/a/test"));
		assert!(regex.is_match("root/a/b/c/test"));
		assert!(!regex.is_match("root/file"));
		assert!(!regex.is_match("root/xxtest"));
	}

	#[test]
	fn doublestar_adjacent_after() {
		let regex = Rule::new("root/test**").unwrap();

		assert!(regex.is_match("root/test"));
		assert!(regex.is_match("root/test/a"));
		assert!(regex.is_match("root/test/a/b/c"));
		assert!(!regex.is_match("root/testxx"));
		assert!(!regex.is_match("root/file"));
	}

	#[test]
	fn doublestar_adjacent_middle() {
		let regex = Rule::new("root/test**file").unwrap();

		assert!(regex.is_match("root/test/file"));
		assert!(regex.is_match("root/test/a/b/c/file"));
		assert!(!regex.is_match("root/test"));
		assert!(!regex.is_match("root/file"));
		assert!(!regex.is_match("root/testfile"));
		assert!(!regex.is_match("root/testxxfile"));
	}

	#[test]
	fn doublestar_nullable() {
		let regex = Rule::new("root/**/file").unwrap();

		assert!(regex.is_match("root/test/file"));
		assert!(regex.is_match("root/file"));
		assert!(!regex.is_match("rootfile"));
	}

	#[test]
	fn doublestar_nullable_post() {
		let regex = Rule::new("root/**").unwrap();

		assert!(regex.is_match("root"));
		assert!(regex.is_match("root/file"));
		assert!(!regex.is_match("rootfile"));
	}

	#[test]
	fn doublestar_nullable_pre() {
		let regex = Rule::new("**/file").unwrap();

		assert!(regex.is_match("file"));
		assert!(regex.is_match("root/file"));
		assert!(!regex.is_match("rootfile"));
	}

	#[test]
	fn doublestar_bad_extension() {
		let regex = Rule::new("**.flac").unwrap();

		assert!(regex.is_match("root/.flac"));
		assert!(regex.is_match("root/a/.flac"));
		assert!(!regex.is_match("root/test.flac"));
		assert!(!regex.is_match("test.flac"));
		assert!(!regex.is_match("root/test/a/b/c.flac"));
		assert!(!regex.is_match("root/testflac"));
		assert!(!regex.is_match("test.mp3"));
	}

	#[test]
	fn doublestar_good_extension() {
		let regex = Rule::new("**/*.flac").unwrap();

		assert!(regex.is_match("root/.flac"));
		assert!(regex.is_match("root/a/.flac"));
		assert!(regex.is_match("root/test.flac"));
		assert!(regex.is_match("test.flac"));
		assert!(regex.is_match("root/test/a/b/c.flac"));
		assert!(!regex.is_match("root/testflac"));
		assert!(!regex.is_match("test.mp3"));
	}

	#[test]
	fn multi_slash_a() {
		let regex = Rule::new("dir//file.txt").unwrap();

		assert!(regex.is_match("dir/file.txt"));
		assert!(!regex.is_match("dirfile.txt"));
		assert!(!regex.is_match("dir/other.txt"));
	}

	#[test]
	fn multi_slash_b() {
		let regex = Rule::new("**///*.txt").unwrap();

		assert!(regex.is_match("dir/file.txt"));
		assert!(regex.is_match("dir/subdir/file.txt"));
		assert!(!regex.is_match("file.jpg"));
	}

	#[test]
	fn multi_slash_c() {
		let regex = Rule::new("///dir//**//*.txt//").unwrap();

		assert!(regex.is_match("dir/subdir/file.txt"));
		assert!(regex.is_match("dir/sub1/sub2/file.txt"));
		assert!(!regex.is_match("other/sub/file.txt"));
		assert!(!regex.is_match("dir/file.jpg"));
	}
}
