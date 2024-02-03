use alloc::string::String;
use alloc::vec::Vec;

use crate::yaml::{EventData, NodeData};
use crate::{
    Break, Document, Emitter, EmitterState, Encoding, Event, MappingStyle, Mark, Node, NodePair,
    Parser, ParserState, ScalarStyle, SequenceStyle, TagDirective, VersionDirective,
    DEFAULT_MAPPING_TAG, DEFAULT_SCALAR_TAG, DEFAULT_SEQUENCE_TAG,
};
use std::collections::VecDeque;

pub(crate) const INPUT_RAW_BUFFER_SIZE: usize = 16384;
pub(crate) const INPUT_BUFFER_SIZE: usize = INPUT_RAW_BUFFER_SIZE;
pub(crate) const OUTPUT_BUFFER_SIZE: usize = 16384;

/// Initialize a parser.
///
/// This function creates a new parser object. An application is responsible
/// for destroying the object using the yaml_parser_delete() function.
pub fn yaml_parser_new<'r>() -> Parser<'r> {
    Parser {
        read_handler: None,
        eof: false,
        buffer: VecDeque::with_capacity(INPUT_BUFFER_SIZE),
        unread: 0,
        encoding: Encoding::Any,
        offset: 0,
        mark: Mark::default(),
        stream_start_produced: false,
        stream_end_produced: false,
        flow_level: 0,
        tokens: VecDeque::with_capacity(16),
        tokens_parsed: 0,
        token_available: false,
        indents: Vec::with_capacity(16),
        indent: 0,
        simple_key_allowed: false,
        simple_keys: Vec::with_capacity(16),
        states: Vec::with_capacity(16),
        state: ParserState::default(),
        marks: Vec::with_capacity(16),
        tag_directives: Vec::with_capacity(16),
        aliases: Vec::new(),
    }
}

/// Reset the parser state.
pub fn yaml_parser_reset(parser: &mut Parser) {
    *parser = yaml_parser_new();
}

/// Set a string input.
pub fn yaml_parser_set_input_string<'r>(parser: &mut Parser<'r>, input: &'r mut &[u8]) {
    assert!((parser.read_handler).is_none());
    parser.read_handler = Some(input);
}

/// Set a generic input handler.
pub fn yaml_parser_set_input<'r>(parser: &mut Parser<'r>, input: &'r mut dyn std::io::BufRead) {
    assert!((parser.read_handler).is_none());
    parser.read_handler = Some(input);
}

/// Set the source encoding.
pub fn yaml_parser_set_encoding(parser: &mut Parser, encoding: Encoding) {
    assert!(parser.encoding == Encoding::Any);
    parser.encoding = encoding;
}

/// Create an emitter.
pub fn yaml_emitter_new<'w>() -> Emitter<'w> {
    Emitter {
        write_handler: None,
        buffer: String::with_capacity(OUTPUT_BUFFER_SIZE),
        raw_buffer: Vec::with_capacity(OUTPUT_BUFFER_SIZE),
        encoding: Encoding::Any,
        canonical: false,
        best_indent: 0,
        best_width: 0,
        unicode: false,
        line_break: Break::default(),
        states: Vec::with_capacity(16),
        state: EmitterState::default(),
        events: VecDeque::with_capacity(16),
        indents: Vec::with_capacity(16),
        tag_directives: Vec::with_capacity(16),
        indent: 0,
        flow_level: 0,
        root_context: false,
        sequence_context: false,
        mapping_context: false,
        simple_key_context: false,
        line: 0,
        column: 0,
        whitespace: false,
        indention: false,
        open_ended: 0,
        opened: false,
        closed: false,
        anchors: Vec::new(),
        last_anchor_id: 0,
    }
}

/// Reset the emitter state.
pub fn yaml_emitter_reset(emitter: &mut Emitter) {
    *emitter = yaml_emitter_new();
}

/// Set a string output.
///
/// The emitter will write the output characters to the `output` buffer.
pub fn yaml_emitter_set_output_string<'w>(emitter: &mut Emitter<'w>, output: &'w mut Vec<u8>) {
    assert!(emitter.write_handler.is_none());
    if emitter.encoding == Encoding::Any {
        yaml_emitter_set_encoding(emitter, Encoding::Utf8);
    } else if emitter.encoding != Encoding::Utf8 {
        panic!("cannot output UTF-16 to String")
    }
    output.clear();
    emitter.write_handler = Some(output);
}

/// Set a generic output handler.
pub fn yaml_emitter_set_output<'w>(emitter: &mut Emitter<'w>, handler: &'w mut dyn std::io::Write) {
    assert!(emitter.write_handler.is_none());
    emitter.write_handler = Some(handler);
}

/// Set the output encoding.
pub fn yaml_emitter_set_encoding(emitter: &mut Emitter, encoding: Encoding) {
    assert_eq!(emitter.encoding, Encoding::Any);
    emitter.encoding = encoding;
}

/// Set if the output should be in the "canonical" format as in the YAML
/// specification.
pub fn yaml_emitter_set_canonical(emitter: &mut Emitter, canonical: bool) {
    emitter.canonical = canonical;
}

/// Set the indentation increment.
pub fn yaml_emitter_set_indent(emitter: &mut Emitter, indent: i32) {
    emitter.best_indent = if 1 < indent && indent < 10 { indent } else { 2 };
}

/// Set the preferred line width. -1 means unlimited.
pub fn yaml_emitter_set_width(emitter: &mut Emitter, width: i32) {
    emitter.best_width = if width >= 0 { width } else { -1 };
}

/// Set if unescaped non-ASCII characters are allowed.
pub fn yaml_emitter_set_unicode(emitter: &mut Emitter, unicode: bool) {
    emitter.unicode = unicode;
}

/// Set the preferred line break.
pub fn yaml_emitter_set_break(emitter: &mut Emitter, line_break: Break) {
    emitter.line_break = line_break;
}

/// Create the STREAM-START event.
pub fn yaml_stream_start_event_new(encoding: Encoding) -> Event {
    Event {
        data: EventData::StreamStart { encoding },
        ..Default::default()
    }
}

/// Create the STREAM-END event.
pub fn yaml_stream_end_event_new() -> Event {
    Event {
        data: EventData::StreamEnd,
        ..Default::default()
    }
}

/// Create the DOCUMENT-START event.
///
/// The `implicit` argument is considered as a stylistic parameter and may be
/// ignored by the emitter.
pub fn yaml_document_start_event_new(
    version_directive: Option<VersionDirective>,
    tag_directives_in: &[TagDirective],
    implicit: bool,
) -> Event {
    let tag_directives = Vec::from_iter(tag_directives_in.iter().cloned());

    Event {
        data: EventData::DocumentStart {
            version_directive,
            tag_directives,
            implicit,
        },
        ..Default::default()
    }
}

/// Create the DOCUMENT-END event.
///
/// The `implicit` argument is considered as a stylistic parameter and may be
/// ignored by the emitter.
pub fn yaml_document_end_event_new(implicit: bool) -> Event {
    Event {
        data: EventData::DocumentEnd { implicit },
        ..Default::default()
    }
}

/// Create an ALIAS event.
pub fn yaml_alias_event_new(anchor: &str) -> Event {
    Event {
        data: EventData::Alias {
            anchor: String::from(anchor),
        },
        ..Default::default()
    }
}

/// Create a SCALAR event.
///
/// The `style` argument may be ignored by the emitter.
///
/// Either the `tag` attribute or one of the `plain_implicit` and
/// `quoted_implicit` flags must be set.
///
pub fn yaml_scalar_event_new(
    anchor: Option<&str>,
    tag: Option<&str>,
    value: &str,
    plain_implicit: bool,
    quoted_implicit: bool,
    style: ScalarStyle,
) -> Event {
    let mark = Mark {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };
    let mut anchor_copy: Option<String> = None;
    let mut tag_copy: Option<String> = None;

    if let Some(anchor) = anchor {
        anchor_copy = Some(String::from(anchor));
    }
    if let Some(tag) = tag {
        tag_copy = Some(String::from(tag));
    }

    Event {
        data: EventData::Scalar {
            anchor: anchor_copy,
            tag: tag_copy,
            value: String::from(value),
            plain_implicit,
            quoted_implicit,
            style,
        },
        start_mark: mark,
        end_mark: mark,
    }
}

/// Create a SEQUENCE-START event.
///
/// The `style` argument may be ignored by the emitter.
///
/// Either the `tag` attribute or the `implicit` flag must be set.
pub fn yaml_sequence_start_event_new(
    anchor: Option<&str>,
    tag: Option<&str>,
    implicit: bool,
    style: SequenceStyle,
) -> Event {
    let mut anchor_copy: Option<String> = None;
    let mut tag_copy: Option<String> = None;

    if let Some(anchor) = anchor {
        anchor_copy = Some(String::from(anchor));
    }
    if let Some(tag) = tag {
        tag_copy = Some(String::from(tag));
    }

    Event {
        data: EventData::SequenceStart {
            anchor: anchor_copy,
            tag: tag_copy,
            implicit,
            style,
        },
        ..Default::default()
    }
}

/// Create a SEQUENCE-END event.
pub fn yaml_sequence_end_event_new() -> Event {
    Event {
        data: EventData::SequenceEnd,
        ..Default::default()
    }
}

/// Create a MAPPING-START event.
///
/// The `style` argument may be ignored by the emitter.
///
/// Either the `tag` attribute or the `implicit` flag must be set.
pub fn yaml_mapping_start_event_new(
    anchor: Option<&str>,
    tag: Option<&str>,
    implicit: bool,
    style: MappingStyle,
) -> Event {
    let mut anchor_copy: Option<String> = None;
    let mut tag_copy: Option<String> = None;

    if let Some(anchor) = anchor {
        anchor_copy = Some(String::from(anchor));
    }

    if let Some(tag) = tag {
        tag_copy = Some(String::from(tag));
    }

    Event {
        data: EventData::MappingStart {
            anchor: anchor_copy,
            tag: tag_copy,
            implicit,
            style,
        },
        ..Default::default()
    }
}

/// Create a MAPPING-END event.
pub fn yaml_mapping_end_event_new() -> Event {
    Event {
        data: EventData::MappingEnd,
        ..Default::default()
    }
}

/// Create a YAML document.
pub fn yaml_document_new(
    version_directive: Option<VersionDirective>,
    tag_directives_in: &[TagDirective],
    start_implicit: bool,
    end_implicit: bool,
) -> Document {
    let nodes = Vec::with_capacity(16);
    let tag_directives = Vec::from_iter(tag_directives_in.iter().cloned());

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

/// Delete a YAML document and all its nodes.
pub fn yaml_document_delete(document: &mut Document) {
    document.nodes.clear();
    document.version_directive = None;
    document.tag_directives.clear();
}

/// Get a node of a YAML document.
///
/// Returns the node object or `None` if `index` is out of range.
pub fn yaml_document_get_node(document: &mut Document, index: i32) -> Option<&mut Node> {
    document.nodes.get_mut(index as usize - 1)
}

/// Get the root of a YAML document node.
///
/// The root object is the first object added to the document.
///
/// An empty document produced by the parser signifies the end of a YAML stream.
///
/// Returns the node object or `None` if the document is empty.
pub fn yaml_document_get_root_node(document: &mut Document) -> Option<&mut Node> {
    document.nodes.get_mut(0)
}

/// Create a SCALAR node and attach it to the document.
///
/// The `style` argument may be ignored by the emitter.
///
/// Returns the node id or 0 on error.
#[must_use]
pub fn yaml_document_add_scalar(
    document: &mut Document,
    tag: Option<&str>,
    value: &str,
    style: ScalarStyle,
) -> i32 {
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
    document.nodes.push(node);
    document.nodes.len() as i32
}

/// Create a SEQUENCE node and attach it to the document.
///
/// The `style` argument may be ignored by the emitter.
///
/// Returns the node id, which is a nonzero integer.
#[must_use]
pub fn yaml_document_add_sequence(
    document: &mut Document,
    tag: Option<&str>,
    style: SequenceStyle,
) -> i32 {
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
    document.nodes.push(node);
    document.nodes.len() as i32
}

/// Create a MAPPING node and attach it to the document.
///
/// The `style` argument may be ignored by the emitter.
///
/// Returns the node id, which is a nonzero integer.
#[must_use]
pub fn yaml_document_add_mapping(
    document: &mut Document,
    tag: Option<&str>,
    style: MappingStyle,
) -> i32 {
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

    document.nodes.push(node);
    document.nodes.len() as i32
}

/// Add an item to a SEQUENCE node.
pub fn yaml_document_append_sequence_item(document: &mut Document, sequence: i32, item: i32) {
    assert!(sequence > 0 && sequence as usize - 1 < document.nodes.len());
    assert!(matches!(
        &document.nodes[sequence as usize - 1].data,
        NodeData::Sequence { .. }
    ));
    assert!(item > 0 && item as usize - 1 < document.nodes.len());
    if let NodeData::Sequence { ref mut items, .. } =
        &mut document.nodes[sequence as usize - 1].data
    {
        items.push(item);
    }
}

/// Add a pair of a key and a value to a MAPPING node.
pub fn yaml_document_append_mapping_pair(
    document: &mut Document,
    mapping: i32,
    key: i32,
    value: i32,
) {
    assert!(mapping > 0 && mapping as usize - 1 < document.nodes.len());
    assert!(matches!(
        &document.nodes[mapping as usize - 1].data,
        NodeData::Mapping { .. }
    ));
    assert!(key > 0 && key as usize - 1 < document.nodes.len());
    assert!(value > 0 && value as usize - 1 < document.nodes.len());
    let pair = NodePair { key, value };
    if let NodeData::Mapping { ref mut pairs, .. } = &mut document.nodes[mapping as usize - 1].data
    {
        pairs.push(pair);
    }
}
