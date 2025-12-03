//! This crate provides a declarative macro for defining datapaths.

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{
	Ident, Token, Type,
	parse::{Parse, ParseStream},
	parse_macro_input,
	punctuated::Punctuated,
};

/// Represents a single datapath definition
#[expect(clippy::large_enum_variant)]
enum DatapathDef {
	/// Simple syntax: `struct Name(path/segments);`
	Simple {
		struct_name: Ident,
		segments: Vec<Segment>,
		attrs: Vec<syn::Attribute>,
	},
	/// Schema syntax: `struct Name { pattern: path/segments, schema: Type }`
	WithSchema {
		struct_name: Ident,
		segments: Vec<Segment>,
		schema_type: Type,
		attrs: Vec<syn::Attribute>,
	},
}

/// Represents a segment in a datapath: either a constant or a typed field
#[expect(clippy::large_enum_variant)]
enum Segment {
	Constant(String),
	Typed { name: Ident, ty: Type },
}

impl Parse for DatapathDef {
	fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
		// Parse attributes (like #[doc = "..."])
		let attrs = input.call(syn::Attribute::parse_outer)?;

		// Parse: struct Name
		input.parse::<Token![struct]>()?;
		let struct_name: Ident = input.parse()?;

		// Check if next is '(' or '{'
		let lookahead = input.lookahead1();

		if lookahead.peek(syn::token::Paren) {
			// Simple syntax: struct Name(...)
			let content;
			syn::parenthesized!(content in input);
			let segments = parse_pattern(&content)?;

			Ok(DatapathDef::Simple {
				struct_name,
				segments,
				attrs,
			})
		} else if lookahead.peek(syn::token::Brace) {
			// Schema syntax: struct Name { pattern: ..., schema: ... }
			let content;
			syn::braced!(content in input);

			// Parse fields in any order
			let mut segments = None;
			let mut schema_type = None;

			while !content.is_empty() {
				let field_name: Ident = content.parse()?;
				content.parse::<Token![:]>()?;

				match field_name.to_string().as_str() {
					"pattern" => {
						if segments.is_some() {
							return Err(syn::Error::new_spanned(
								field_name,
								"duplicate 'pattern' field",
							));
						}
						// Find the next field keyword to know where pattern ends
						let next_keyword = find_next_keyword(&content);
						segments = Some(if let Some(kw) = next_keyword {
							parse_pattern_until_keyword(&content, &kw)?
						} else {
							parse_pattern(&content)?
						});
					}
					"schema" => {
						if schema_type.is_some() {
							return Err(syn::Error::new_spanned(
								field_name,
								"duplicate 'schema' field",
							));
						}
						schema_type = Some(content.parse()?);
					}
					_ => {
						return Err(syn::Error::new_spanned(
							field_name,
							"unknown field, expected 'pattern' or 'schema'",
						));
					}
				}
			}

			// Ensure required fields are present
			let segments = segments.ok_or_else(|| {
				syn::Error::new(content.span(), "missing required field 'pattern'")
			})?;
			let schema_type = schema_type.ok_or_else(|| {
				syn::Error::new(content.span(), "missing required field 'schema'")
			})?;

			Ok(DatapathDef::WithSchema {
				struct_name,
				segments,
				schema_type,
				attrs,
			})
		} else {
			Err(lookahead.error())
		}
	}
}

/// Find the next field keyword in the input stream
fn find_next_keyword(input: ParseStream<'_>) -> Option<String> {
	let fork = input.fork();

	// Skip through tokens until we find an identifier that could be a keyword
	while !fork.is_empty() {
		if fork.peek(Ident) {
			if let Ok(ident) = fork.parse::<Ident>() {
				let ident_str = ident.to_string();
				if ident_str == "schema" {
					return Some(ident_str);
				}
			}
		} else {
			// Try to advance past the current token
			let _ = fork.parse::<proc_macro2::TokenTree>();
		}
	}

	None
}

/// Parse a complete pattern (used when the entire input is the pattern)
fn parse_pattern(input: ParseStream<'_>) -> syn::Result<Vec<Segment>> {
	let mut segments = Vec::new();
	let mut current_token = String::new();

	while !input.is_empty() {
		parse_next_segment(input, &mut segments, &mut current_token)?;
	}

	// Add remaining constant if any
	if !current_token.is_empty() {
		segments.push(Segment::Constant(current_token));
	}

	Ok(segments)
}

/// Parse pattern until we encounter a specific keyword (like "schema")
fn parse_pattern_until_keyword(
	input: ParseStream<'_>,
	stop_keyword: &str,
) -> syn::Result<Vec<Segment>> {
	let mut segments = Vec::new();
	let mut current_token = String::new();

	while !input.is_empty() {
		// Check if next token is the stop keyword
		if input.peek(Ident) {
			let fork = input.fork();
			if let Ok(ident) = fork.parse::<Ident>()
				&& ident == stop_keyword
			{
				// Found the stop keyword, finalize and return
				if !current_token.is_empty() {
					segments.push(Segment::Constant(current_token));
				}
				return Ok(segments);
			}
		}

		parse_next_segment(input, &mut segments, &mut current_token)?;
	}

	// Add remaining constant if any
	if !current_token.is_empty() {
		segments.push(Segment::Constant(current_token));
	}

	Ok(segments)
}

/// Parse the next segment in a pattern
fn parse_next_segment(
	input: ParseStream<'_>,
	segments: &mut Vec<Segment>,
	current_token: &mut String,
) -> syn::Result<()> {
	// Try to parse as string literal first (for quoted keys or constants)
	if input.peek(syn::LitStr) {
		let lit: syn::LitStr = input.parse()?;
		let lit_value = lit.value();

		// Check if next token is '=' (quoted partition key)
		if input.peek(Token![=]) {
			input.parse::<Token![=]>()?;
			let ty: Type = input.parse()?;

			// Create an Ident from the string literal value, replacing '-' with '_'
			let ident_str = lit_value.replace('-', "_");
			let ident = Ident::new(&ident_str, lit.span());

			segments.push(Segment::Typed { name: ident, ty });

			// Check for '/' separator
			if input.peek(Token![/]) {
				input.parse::<Token![/]>()?;
			}
		} else {
			// This is a constant segment
			if !current_token.is_empty() {
				current_token.push('/');
			}
			current_token.push_str(&lit_value);

			// Check for '/' separator
			if input.peek(Token![/]) {
				input.parse::<Token![/]>()?;
				segments.push(Segment::Constant(current_token.clone()));
				current_token.clear();
			}
		}
	} else if let Ok(ident) = input.parse::<Ident>() {
		let ident_str = ident.to_string();

		// Check if next token is '='
		if input.peek(Token![=]) {
			input.parse::<Token![=]>()?;
			let ty: Type = input.parse()?;

			segments.push(Segment::Typed {
				name: ident.clone(),
				ty,
			});

			// Check for '/' separator
			if input.peek(Token![/]) {
				input.parse::<Token![/]>()?;
			}
		} else {
			// This is a constant segment
			if !current_token.is_empty() {
				current_token.push('/');
			}
			current_token.push_str(&ident_str);

			// Check for '/' separator
			if input.peek(Token![/]) {
				input.parse::<Token![/]>()?;
				segments.push(Segment::Constant(current_token.clone()));
				current_token.clear();
			}
		}
	} else {
		// Try to parse as literal (for version numbers, string literals, or plain integers)
		let lookahead = input.lookahead1();

		if lookahead.peek(syn::LitStr) {
			// String literal segment like "dashed-path-segment"
			let lit: syn::LitStr = input.parse()?;
			if !current_token.is_empty() {
				current_token.push('/');
			}
			current_token.push_str(&lit.value());

			// Check for '/' separator
			if input.peek(Token![/]) {
				input.parse::<Token![/]>()?;
				segments.push(Segment::Constant(current_token.clone()));
				current_token.clear();
			}
		} else if lookahead.peek(syn::LitFloat) {
			let lit: syn::LitFloat = input.parse()?;
			if !current_token.is_empty() {
				current_token.push('/');
			}
			current_token.push_str(&lit.to_string());

			// Check for '/' separator
			if input.peek(Token![/]) {
				input.parse::<Token![/]>()?;
				segments.push(Segment::Constant(current_token.clone()));
				current_token.clear();
			}
		} else if lookahead.peek(syn::LitInt) {
			let lit: syn::LitInt = input.parse()?;
			if !current_token.is_empty() {
				current_token.push('/');
			}
			current_token.push_str(&lit.to_string());

			// Check for '/' separator
			if input.peek(Token![/]) {
				input.parse::<Token![/]>()?;
				segments.push(Segment::Constant(current_token.clone()));
				current_token.clear();
			}
		} else {
			return Err(lookahead.error());
		}
	}

	Ok(())
}

/// Generate code for a datapath definition
fn generate_datapath_code(def: DatapathDef) -> proc_macro2::TokenStream {
	match def {
		DatapathDef::Simple {
			struct_name,
			segments,
			attrs,
		} => generate_simple_datapath(&struct_name, &segments, &attrs),
		DatapathDef::WithSchema {
			struct_name,
			segments,
			schema_type,
			attrs,
		} => generate_schema_datapath(&struct_name, &segments, &schema_type, &attrs),
	}
}

/// Generate code for simple datapath (without schema)
fn generate_simple_datapath(
	struct_name: &Ident,
	segments: &[Segment],
	attrs: &[syn::Attribute],
) -> proc_macro2::TokenStream {
	let (struct_def, display_impl, datapath_impl) =
		generate_common_impls(struct_name, segments, attrs);

	quote! {
		#struct_def
		#display_impl
		#datapath_impl
	}
}

/// Generate code for datapath with schema
fn generate_schema_datapath(
	struct_name: &Ident,
	segments: &[Segment],
	schema_type: &Type,
	attrs: &[syn::Attribute],
) -> proc_macro2::TokenStream {
	let (struct_def, display_impl, datapath_impl) =
		generate_common_impls(struct_name, segments, attrs);

	// Generate SchemaDatapath implementation
	let schema_datapath_impl = quote! {
		impl ::datapath::SchemaDatapath for #struct_name {
			type Schema = #schema_type;
		}
	};

	quote! {
		#struct_def
		#display_impl
		#datapath_impl
		#schema_datapath_impl
	}
}

/// Generate common implementations shared by both variants
fn generate_common_impls(
	struct_name: &Ident,
	segments: &[Segment],
	attrs: &[syn::Attribute],
) -> (
	proc_macro2::TokenStream,
	proc_macro2::TokenStream,
	proc_macro2::TokenStream,
) {
	// Extract typed fields
	let typed_fields: Vec<_> = segments
		.iter()
		.filter_map(|seg| match seg {
			Segment::Typed { name, ty } => Some((name, ty)),
			_ => None,
		})
		.collect();

	// Generate struct fields
	let struct_fields = typed_fields.iter().map(|(name, ty)| {
		quote! {
			pub #name: #ty
		}
	});

	let mut doc_str = String::new();
	for s in segments {
		if !doc_str.is_empty() {
			doc_str.push('/');
		}

		match s {
			Segment::Constant(x) => doc_str.push_str(x),
			Segment::Typed { name, ty } => {
				doc_str.push_str(&format!("{name}={}", ty.to_token_stream()))
			}
		}
	}

	let doc_str = format!("\n\nDatapath pattern: `{doc_str}`");

	let struct_def = quote! {
		#(#attrs)*
		#[allow(non_camel_case_types)]
		#[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash)]
		#[doc = #doc_str]
		pub struct #struct_name {
			#(#struct_fields),*
		}
	};

	// Generate Display implementation
	let display_parts = segments.iter().map(|seg| match seg {
		Segment::Constant(s) => quote! { #s.to_string() },
		Segment::Typed { name, .. } => quote! { format!("{}={}", stringify!(#name), self.#name) },
	});

	let display_impl = quote! {
		impl ::core::fmt::Display for #struct_name {
			fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
				write!(f, "{}", vec![#(#display_parts),*].join("/"))
			}
		}
	};

	// Generate parse implementation
	let mut parse_body = Vec::new();

	for seg in segments {
		match seg {
			Segment::Constant(s) => {
				parse_body.push(quote! {
					{
						match parts.next() {
							Option::Some(#s) => {}
							_ => return Option::None,
						}
					}
				});
			}
			Segment::Typed { name, ty } => {
				let name_str = name.to_string();
				parse_body.push(quote! {
					let #name: #ty = {
						let x = match parts.next() {
							Option::Some(x) => x.strip_prefix(concat!(#name_str, "="))?,
							_ => return Option::None,
						};

						::core::str::FromStr::from_str(x).ok()?
					};
				});
			}
		}
	}

	// Extract just the field names for struct construction
	let field_names = typed_fields.iter().map(|(name, _)| name);

	let datapath_impl = quote! {
		impl ::datapath::Datapath for #struct_name {
			fn with_file(&self, file: impl ::core::convert::Into<::std::string::String>) -> ::datapath::DatapathFile<Self> {
				::datapath::DatapathFile {
					path: self.clone(),
					file: file.into(),
				}
			}

			fn parse(path: &str) -> Option<::datapath::DatapathFile<Self>> {
				if path.contains("\n") {
					return Option::None;
				}

				let mut parts = path.split("/");

				#(#parse_body)*

				let mut file = ::std::string::String::new();
				if let Option::Some(first) = parts.next() {
					file.push_str(first);
					for part in parts {
						file.push_str("/");
						file.push_str(part);
					}
				}

				Option::Some(::datapath::DatapathFile {
					path: Self { #(#field_names),* },
					file,
				})
			}
		}
	};

	(struct_def, display_impl, datapath_impl)
}

/// The `datapath!` macro generates datapath struct definitions with parsing and formatting logic.
///
/// # Example
/// ```ignore
/// datapath! {
///     struct CaptureRaw_2_0(capture/user_id=Uuid/ts=i64/raw/2.0);
///     struct OtherPath {
///         pattern: web/domain=String/ts=i64/raw/2.0
///         schema: MySchema
///     };
/// }
/// ```
#[proc_macro]
pub fn datapath(input: TokenStream) -> TokenStream {
	let defs =
		parse_macro_input!(input with Punctuated::<DatapathDef, Token![;]>::parse_terminated);

	let generated = defs.into_iter().map(generate_datapath_code);

	let output = quote! {
		#(#generated)*
	};

	output.into()
}
