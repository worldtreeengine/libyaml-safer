use crate::{
    MappingStyle, Mark, ScalarStyle, SequenceStyle, TagDirective, VersionDirective,
    DEFAULT_MAPPING_TAG, DEFAULT_SCALAR_TAG, DEFAULT_SEQUENCE_TAG,
};

/// The document structure.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Document {
    /// The document nodes.
    pub nodes: Vec<Node>,
    /// The version directive.
    pub version_directive: Option<VersionDirective>,
    /// The list of tag directives.
    ///
    /// ```
    /// # const _: &str = stringify! {
    /// struct {
    ///     /// The beginning of the tag directives list.
    ///     start: *mut yaml_tag_directive_t,
    ///     /// The end of the tag directives list.
    ///     end: *mut yaml_tag_directive_t,
    /// }
    /// # };
    /// ```
    pub tag_directives: Vec<TagDirective>,
    /// Is the document start indicator implicit?
    pub start_implicit: bool,
    /// Is the document end indicator implicit?
    pub end_implicit: bool,
    /// The beginning of the document.
    pub start_mark: Mark,
    /// The end of the document.
    pub end_mark: Mark,
}

/// The node structure.
#[derive(Clone, Default, Debug)]
#[non_exhaustive]
pub struct Node {
    /// The node type.
    pub data: NodeData,
    /// The node tag.
    pub tag: Option<String>,
    /// The beginning of the node.
    pub start_mark: Mark,
    /// The end of the node.
    pub end_mark: Mark,
}

/// Node types.
#[derive(Clone, Default, Debug)]
pub enum NodeData {
    /// An empty node.
    #[default]
    NoNode,
    /// A scalar node.
    Scalar {
        /// The scalar value.
        value: String,
        /// The scalar style.
        style: ScalarStyle,
    },
    /// A sequence node.
    Sequence {
        /// The stack of sequence items.
        items: Vec<NodeItem>,
        /// The sequence style.
        style: SequenceStyle,
    },
    /// A mapping node.
    Mapping {
        /// The stack of mapping pairs (key, value).
        pairs: Vec<NodePair>,
        /// The mapping style.
        style: MappingStyle,
    },
}

/// An element of a sequence node.
pub type NodeItem = i32;

/// An element of a mapping node.
#[derive(Copy, Clone, Default, Debug)]
#[non_exhaustive]
pub struct NodePair {
    /// The key of the element.
    pub key: i32,
    /// The value of the element.
    pub value: i32,
}

impl Document {
    /// Create a YAML document.
    pub fn new(
        version_directive: Option<VersionDirective>,
        tag_directives_in: &[TagDirective],
        start_implicit: bool,
        end_implicit: bool,
    ) -> Document {
        let nodes = Vec::with_capacity(16);
        let tag_directives = tag_directives_in.to_vec();

        Document {
            nodes,
            version_directive,
            tag_directives,
            start_implicit,
            end_implicit,
            start_mark: Mark::default(),
            end_mark: Mark::default(),
        }
    }

    /// Get a node of a YAML document.
    ///
    /// Returns the node object or `None` if `index` is out of range.
    pub fn get_node_mut(&mut self, index: i32) -> Option<&mut Node> {
        self.nodes.get_mut(index as usize - 1)
    }

    /// Get a node of a YAML document.
    ///
    /// Returns the node object or `None` if `index` is out of range.
    pub fn get_node(&self, index: i32) -> Option<&Node> {
        self.nodes.get(index as usize - 1)
    }

    /// Get the root of a YAML document node.
    ///
    /// The root object is the first object added to the document.
    ///
    /// An empty document produced by the parser signifies the end of a YAML stream.
    ///
    /// Returns the node object or `None` if the document is empty.
    pub fn get_root_node(&mut self) -> Option<&mut Node> {
        self.nodes.get_mut(0)
    }

    /// Create a SCALAR node and attach it to the document.
    ///
    /// The `style` argument may be ignored by the emitter.
    ///
    /// Returns the node id or 0 on error.
    #[must_use]
    pub fn add_scalar(&mut self, tag: Option<&str>, value: &str, style: ScalarStyle) -> i32 {
        let mark = Mark {
            index: 0_u64,
            line: 0_u64,
            column: 0_u64,
        };
        let tag = tag.unwrap_or(DEFAULT_SCALAR_TAG);
        let tag_copy = String::from(tag);
        let value_copy = String::from(value);
        let node = Node {
            data: NodeData::Scalar {
                value: value_copy,
                style,
            },
            tag: Some(tag_copy),
            start_mark: mark,
            end_mark: mark,
        };
        self.nodes.push(node);
        self.nodes.len() as i32
    }

    /// Create a SEQUENCE node and attach it to the document.
    ///
    /// The `style` argument may be ignored by the emitter.
    ///
    /// Returns the node id, which is a nonzero integer.
    #[must_use]
    pub fn add_sequence(&mut self, tag: Option<&str>, style: SequenceStyle) -> i32 {
        let mark = Mark {
            index: 0_u64,
            line: 0_u64,
            column: 0_u64,
        };

        let items = Vec::with_capacity(16);
        let tag = tag.unwrap_or(DEFAULT_SEQUENCE_TAG);
        let tag_copy = String::from(tag);
        let node = Node {
            data: NodeData::Sequence { items, style },
            tag: Some(tag_copy),
            start_mark: mark,
            end_mark: mark,
        };
        self.nodes.push(node);
        self.nodes.len() as i32
    }

    /// Create a MAPPING node and attach it to the document.
    ///
    /// The `style` argument may be ignored by the emitter.
    ///
    /// Returns the node id, which is a nonzero integer.
    #[must_use]
    pub fn add_mapping(&mut self, tag: Option<&str>, style: MappingStyle) -> i32 {
        let mark = Mark {
            index: 0_u64,
            line: 0_u64,
            column: 0_u64,
        };
        let pairs = Vec::with_capacity(16);
        let tag = tag.unwrap_or(DEFAULT_MAPPING_TAG);
        let tag_copy = String::from(tag);

        let node = Node {
            data: NodeData::Mapping { pairs, style },
            tag: Some(tag_copy),
            start_mark: mark,
            end_mark: mark,
        };

        self.nodes.push(node);
        self.nodes.len() as i32
    }

    /// Add an item to a SEQUENCE node.
    pub fn append_sequence_item(&mut self, sequence: i32, item: i32) {
        assert!(sequence > 0 && sequence as usize - 1 < self.nodes.len());
        assert!(matches!(
            &self.nodes[sequence as usize - 1].data,
            NodeData::Sequence { .. }
        ));
        assert!(item > 0 && item as usize - 1 < self.nodes.len());
        if let NodeData::Sequence { ref mut items, .. } =
            &mut self.nodes[sequence as usize - 1].data
        {
            items.push(item);
        }
    }

    /// Add a pair of a key and a value to a MAPPING node.
    pub fn yaml_document_append_mapping_pair(&mut self, mapping: i32, key: i32, value: i32) {
        assert!(mapping > 0 && mapping as usize - 1 < self.nodes.len());
        assert!(matches!(
            &self.nodes[mapping as usize - 1].data,
            NodeData::Mapping { .. }
        ));
        assert!(key > 0 && key as usize - 1 < self.nodes.len());
        assert!(value > 0 && value as usize - 1 < self.nodes.len());
        let pair = NodePair { key, value };
        if let NodeData::Mapping { ref mut pairs, .. } = &mut self.nodes[mapping as usize - 1].data
        {
            pairs.push(pair);
        }
    }
}
