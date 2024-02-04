use crate::{
    AliasData, ComposerError, Event, EventData, MappingStyle, Mark, Parser, ScalarStyle,
    SequenceStyle, TagDirective, VersionDirective, DEFAULT_MAPPING_TAG, DEFAULT_SCALAR_TAG,
    DEFAULT_SEQUENCE_TAG,
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

    /// Parse the input stream and produce the next YAML document.
    ///
    /// Call this function subsequently to produce a sequence of documents
    /// constituting the input stream.
    ///
    /// If the produced document has no root node, it means that the document end
    /// has been reached.
    ///
    /// An application must not alternate the calls of
    /// [`yaml_parser_load()`](crate::yaml_parser_load) with the calls of
    /// [`yaml_parser_scan()`](crate::yaml_parser_scan) or
    /// [`yaml_parser_parse()`](crate::yaml_parser_parse). Doing this will break the
    /// parser.
    pub fn load(parser: &mut Parser) -> Result<Document, ComposerError> {
        let mut document = Document::new(None, &[], false, false);
        document.nodes.reserve(16);

        if !parser.stream_start_produced {
            match parser.parse() {
                Ok(Event {
                    data: EventData::StreamStart { .. },
                    ..
                }) => (),
                Ok(_) => panic!("expected stream start"),
                Err(err) => {
                    parser.delete_aliases();
                    return Err(err.into());
                }
            }
        }
        if parser.stream_end_produced {
            return Ok(document);
        }
        let err: ComposerError;
        match parser.parse() {
            Ok(event) => {
                if let EventData::StreamEnd = &event.data {
                    return Ok(document);
                }
                parser.aliases.reserve(16);
                match document.load_document(parser, event) {
                    Ok(()) => {
                        parser.delete_aliases();
                        return Ok(document);
                    }
                    Err(e) => err = e,
                }
            }
            Err(e) => err = e.into(),
        }
        parser.delete_aliases();
        Err(err)
    }

    fn set_composer_error<T>(
        problem: &'static str,
        problem_mark: Mark,
    ) -> Result<T, ComposerError> {
        Err(ComposerError::Problem {
            problem,
            mark: problem_mark,
        })
    }

    fn set_composer_error_context<T>(
        context: &'static str,
        context_mark: Mark,
        problem: &'static str,
        problem_mark: Mark,
    ) -> Result<T, ComposerError> {
        Err(ComposerError::ProblemWithContext {
            context,
            context_mark,
            problem,
            mark: problem_mark,
        })
    }

    fn load_document(&mut self, parser: &mut Parser, event: Event) -> Result<(), ComposerError> {
        let mut ctx = vec![];
        if let EventData::DocumentStart {
            version_directive,
            tag_directives,
            implicit,
        } = event.data
        {
            self.version_directive = version_directive;
            self.tag_directives = tag_directives;
            self.start_implicit = implicit;
            self.start_mark = event.start_mark;
            ctx.reserve(16);
            if let Err(err) = self.load_nodes(parser, &mut ctx) {
                ctx.clear();
                return Err(err);
            }
            ctx.clear();
            Ok(())
        } else {
            panic!("Expected YAML_DOCUMENT_START_EVENT")
        }
    }

    fn load_nodes(&mut self, parser: &mut Parser, ctx: &mut Vec<i32>) -> Result<(), ComposerError> {
        let end_implicit;
        let end_mark;

        loop {
            let event = parser.parse()?;
            match event.data {
                EventData::NoEvent => panic!("empty event"),
                EventData::StreamStart { .. } => panic!("unexpected stream start event"),
                EventData::StreamEnd => panic!("unexpected stream end event"),
                EventData::DocumentStart { .. } => panic!("unexpected document start event"),
                EventData::DocumentEnd { implicit } => {
                    end_implicit = implicit;
                    end_mark = event.end_mark;
                    break;
                }
                EventData::Alias { .. } => {
                    self.load_alias(parser, event, ctx)?;
                }
                EventData::Scalar { .. } => {
                    self.load_scalar(parser, event, ctx)?;
                }
                EventData::SequenceStart { .. } => {
                    self.load_sequence(parser, event, ctx)?;
                }
                EventData::SequenceEnd => {
                    self.load_sequence_end(event, ctx)?;
                }
                EventData::MappingStart { .. } => {
                    self.load_mapping(parser, event, ctx)?;
                }
                EventData::MappingEnd => {
                    self.load_mapping_end(event, ctx)?;
                }
            }
        }
        self.end_implicit = end_implicit;
        self.end_mark = end_mark;
        Ok(())
    }

    fn register_anchor(
        &mut self,
        parser: &mut Parser,
        index: i32,
        anchor: Option<String>,
    ) -> Result<(), ComposerError> {
        let Some(anchor) = anchor else {
            return Ok(());
        };
        let data = AliasData {
            anchor,
            index,
            mark: self.nodes[index as usize - 1].start_mark,
        };
        for alias_data in &parser.aliases {
            if alias_data.anchor == data.anchor {
                return Self::set_composer_error_context(
                    "found duplicate anchor; first occurrence",
                    alias_data.mark,
                    "second occurrence",
                    data.mark,
                );
            }
        }
        parser.aliases.push(data);
        Ok(())
    }

    fn load_node_add(&mut self, ctx: &[i32], index: i32) -> Result<(), ComposerError> {
        if ctx.is_empty() {
            return Ok(());
        }
        let parent_index: i32 = *ctx.last().unwrap();
        let parent = &mut self.nodes[parent_index as usize - 1];
        match parent.data {
            NodeData::Sequence { ref mut items, .. } => {
                items.push(index);
            }
            NodeData::Mapping { ref mut pairs, .. } => {
                let mut pair = NodePair::default();
                let mut do_push = true;
                if !pairs.is_empty() {
                    let p: &mut NodePair = pairs.last_mut().unwrap();
                    if p.key != 0 && p.value == 0 {
                        p.value = index;
                        do_push = false;
                    }
                }
                if do_push {
                    pair.key = index;
                    pair.value = 0;
                    pairs.push(pair);
                }
            }
            _ => {
                panic!("document parent node is not a sequence or a mapping")
            }
        }
        Ok(())
    }

    fn load_alias(
        &mut self,
        parser: &mut Parser,
        event: Event,
        ctx: &[i32],
    ) -> Result<(), ComposerError> {
        let EventData::Alias { anchor } = &event.data else {
            unreachable!()
        };

        for alias_data in &parser.aliases {
            if alias_data.anchor == *anchor {
                return self.load_node_add(ctx, alias_data.index);
            }
        }

        Self::set_composer_error("found undefined alias", event.start_mark)
    }

    fn load_scalar(
        &mut self,
        parser: &mut Parser,
        event: Event,
        ctx: &[i32],
    ) -> Result<(), ComposerError> {
        let EventData::Scalar {
            mut tag,
            value,
            style,
            anchor,
            ..
        } = event.data
        else {
            unreachable!()
        };

        if tag.is_none() || tag.as_deref() == Some("!") {
            tag = Some(String::from(DEFAULT_SCALAR_TAG));
        }
        let node = Node {
            data: NodeData::Scalar { value, style },
            tag,
            start_mark: event.start_mark,
            end_mark: event.end_mark,
        };
        self.nodes.push(node);
        let index: i32 = self.nodes.len() as i32;
        self.register_anchor(parser, index, anchor)?;
        self.load_node_add(ctx, index)
    }

    fn load_sequence(
        &mut self,
        parser: &mut Parser,
        event: Event,
        ctx: &mut Vec<i32>,
    ) -> Result<(), ComposerError> {
        let EventData::SequenceStart {
            anchor,
            mut tag,
            style,
            ..
        } = event.data
        else {
            unreachable!()
        };

        let mut items = Vec::with_capacity(16);

        if tag.is_none() || tag.as_deref() == Some("!") {
            tag = Some(String::from(DEFAULT_SEQUENCE_TAG));
        }

        let node = Node {
            data: NodeData::Sequence {
                items: core::mem::take(&mut items),
                style,
            },
            tag,
            start_mark: event.start_mark,
            end_mark: event.end_mark,
        };

        self.nodes.push(node);
        let index: i32 = self.nodes.len() as i32;
        self.register_anchor(parser, index, anchor)?;
        self.load_node_add(ctx, index)?;
        ctx.push(index);
        Ok(())
    }

    fn load_sequence_end(&mut self, event: Event, ctx: &mut Vec<i32>) -> Result<(), ComposerError> {
        assert!(!ctx.is_empty());
        let index: i32 = *ctx.last().unwrap();
        assert!(matches!(
            self.nodes[index as usize - 1].data,
            NodeData::Sequence { .. }
        ));
        self.nodes[index as usize - 1].end_mark = event.end_mark;
        _ = ctx.pop();
        Ok(())
    }

    fn load_mapping(
        &mut self,
        parser: &mut Parser,
        event: Event,
        ctx: &mut Vec<i32>,
    ) -> Result<(), ComposerError> {
        let EventData::MappingStart {
            anchor,
            mut tag,
            style,
            ..
        } = event.data
        else {
            unreachable!()
        };

        let mut pairs = Vec::with_capacity(16);

        if tag.is_none() || tag.as_deref() == Some("!") {
            tag = Some(String::from(DEFAULT_MAPPING_TAG));
        }
        let node = Node {
            data: NodeData::Mapping {
                pairs: core::mem::take(&mut pairs),
                style,
            },
            tag,
            start_mark: event.start_mark,
            end_mark: event.end_mark,
        };
        self.nodes.push(node);
        let index: i32 = self.nodes.len() as i32;
        self.register_anchor(parser, index, anchor)?;
        self.load_node_add(ctx, index)?;
        ctx.push(index);
        Ok(())
    }

    fn load_mapping_end(&mut self, event: Event, ctx: &mut Vec<i32>) -> Result<(), ComposerError> {
        assert!(!ctx.is_empty());
        let index: i32 = *ctx.last().unwrap();
        assert!(matches!(
            self.nodes[index as usize - 1].data,
            NodeData::Mapping { .. }
        ));
        self.nodes[index as usize - 1].end_mark = event.end_mark;
        _ = ctx.pop();
        Ok(())
    }
}
