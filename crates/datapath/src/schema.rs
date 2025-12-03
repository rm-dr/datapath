use crate::Datapath;

/// A datapath with an associated schema.
///
/// Provides [AssociatedDatapath::Schema],
/// which is the schema that is available at this path.
///
/// A datapath can have at most one schema, but the same
/// schema may be used in an arbitrary number of datapaths.
pub trait SchemaDatapath
where
	Self: Datapath,
{
	type Schema;
}
