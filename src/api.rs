use alloc::string::String;
use alloc::vec::Vec;

use crate::yaml::{YamlEventData, YamlNodeData};
use crate::{
    libc, yaml_break_t, yaml_document_t, yaml_emitter_state_t, yaml_emitter_t, yaml_encoding_t,
    yaml_event_t, yaml_mapping_style_t, yaml_mark_t, yaml_node_pair_t, yaml_node_t,
    yaml_parser_state_t, yaml_parser_t, yaml_scalar_style_t, yaml_sequence_style_t,
    yaml_tag_directive_t, yaml_token_t, yaml_version_directive_t, YAML_ANY_ENCODING,
    YAML_UTF8_ENCODING,
};
use core::ptr;
use std::collections::VecDeque;

pub(crate) const INPUT_RAW_BUFFER_SIZE: usize = 16384;
pub(crate) const INPUT_BUFFER_SIZE: usize = INPUT_RAW_BUFFER_SIZE;
pub(crate) const OUTPUT_BUFFER_SIZE: usize = 16384;

/// Initialize a parser.
///
/// This function creates a new parser object. An application is responsible
/// for destroying the object using the yaml_parser_delete() function.
pub fn yaml_parser_new<'r>() -> yaml_parser_t<'r> {
    yaml_parser_t {
        read_handler: None,
        input: Default::default(),
        eof: false,
        buffer: VecDeque::with_capacity(INPUT_BUFFER_SIZE),
        unread: 0,
        raw_buffer: VecDeque::with_capacity(INPUT_RAW_BUFFER_SIZE),
        encoding: YAML_ANY_ENCODING,
        offset: 0,
        mark: yaml_mark_t::default(),
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
        state: yaml_parser_state_t::default(),
        marks: Vec::with_capacity(16),
        tag_directives: Vec::with_capacity(16),
        aliases: Vec::new(),
    }
}

/// Destroy a parser.
pub fn yaml_parser_delete(parser: &mut yaml_parser_t) {
    parser.raw_buffer.clear();
    parser.buffer.clear();
    for mut token in parser.tokens.drain(..) {
        yaml_token_delete(&mut token);
    }
    parser.indents.clear();
    parser.simple_keys.clear();
    parser.states.clear();
    parser.marks.clear();
    parser.tag_directives.clear();
}

/// Set a string input.
///
/// Note that the `input` pointer must be valid while the `parser` object
/// exists. The application is responsible for destroying `input` after
/// destroying the `parser`.
pub fn yaml_parser_set_input_string<'r>(parser: &mut yaml_parser_t<'r>, input: &'r mut &[u8]) {
    __assert!((parser.read_handler).is_none());
    parser.read_handler = Some(input);
}

/// Set a generic input handler.
pub fn yaml_parser_set_input<'r>(parser: &mut yaml_parser_t<'r>, input: &'r mut dyn std::io::Read) {
    __assert!((parser.read_handler).is_none());
    parser.read_handler = Some(input);
}

/// Set the source encoding.
pub fn yaml_parser_set_encoding(parser: &mut yaml_parser_t, encoding: yaml_encoding_t) {
    __assert!(parser.encoding == YAML_ANY_ENCODING);
    parser.encoding = encoding;
}

/// Initialize an emitter.
///
/// This function creates a new emitter object. An application is responsible
/// for destroying the object using the yaml_emitter_delete() function.
pub fn yaml_emitter_new<'w>() -> yaml_emitter_t<'w> {
    yaml_emitter_t {
        write_handler: None,
        output: Default::default(),
        buffer: String::with_capacity(OUTPUT_BUFFER_SIZE),
        raw_buffer: Vec::with_capacity(OUTPUT_BUFFER_SIZE),
        encoding: YAML_ANY_ENCODING,
        canonical: false,
        best_indent: 0,
        best_width: 0,
        unicode: false,
        line_break: yaml_break_t::default(),
        states: Vec::with_capacity(16),
        state: yaml_emitter_state_t::default(),
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

/// Destroy an emitter.
pub fn yaml_emitter_delete(emitter: &mut yaml_emitter_t) {
    emitter.buffer.clear();
    emitter.raw_buffer.clear();
    emitter.states.clear();
    while let Some(mut event) = emitter.events.pop_front() {
        yaml_event_delete(&mut event);
    }
    emitter.indents.clear();
    emitter.tag_directives.clear();
    *emitter = yaml_emitter_t::default();
}

/// Set a string output.
///
/// The emitter will write the output characters to the `output` buffer of the
/// size `size`. The emitter will set `size_written` to the number of written
/// bytes. If the buffer is smaller than required, the emitter produces the
/// YAML_WRITE_ERROR error.
pub fn yaml_emitter_set_output_string<'w>(
    emitter: &mut yaml_emitter_t<'w>,
    output: &'w mut Vec<u8>,
) {
    __assert!(emitter.write_handler.is_none());
    if emitter.encoding == YAML_ANY_ENCODING {
        yaml_emitter_set_encoding(emitter, YAML_UTF8_ENCODING);
    } else if emitter.encoding != YAML_UTF8_ENCODING {
        panic!("cannot output UTF-16 to String")
    }
    output.clear();
    emitter.write_handler = Some(output);
}

/// Set a generic output handler.
pub fn yaml_emitter_set_output<'w>(
    emitter: &mut yaml_emitter_t<'w>,
    handler: &'w mut dyn std::io::Write,
) {
    __assert!(emitter.write_handler.is_none());
    emitter.write_handler = Some(handler);
}

/// Set the output encoding.
pub fn yaml_emitter_set_encoding(emitter: &mut yaml_emitter_t, encoding: yaml_encoding_t) {
    __assert!(emitter.encoding == YAML_ANY_ENCODING);
    emitter.encoding = encoding;
}

/// Set if the output should be in the "canonical" format as in the YAML
/// specification.
pub fn yaml_emitter_set_canonical(emitter: &mut yaml_emitter_t, canonical: bool) {
    emitter.canonical = canonical;
}

/// Set the indentation increment.
pub fn yaml_emitter_set_indent(emitter: &mut yaml_emitter_t, indent: libc::c_int) {
    emitter.best_indent = if 1 < indent && indent < 10 { indent } else { 2 };
}

/// Set the preferred line width. -1 means unlimited.
pub fn yaml_emitter_set_width(emitter: &mut yaml_emitter_t, width: libc::c_int) {
    emitter.best_width = if width >= 0 { width } else { -1 };
}

/// Set if unescaped non-ASCII characters are allowed.
pub fn yaml_emitter_set_unicode(emitter: &mut yaml_emitter_t, unicode: bool) {
    emitter.unicode = unicode;
}

/// Set the preferred line break.
pub fn yaml_emitter_set_break(emitter: &mut yaml_emitter_t, line_break: yaml_break_t) {
    emitter.line_break = line_break;
}

/// Free any memory allocated for a token object.
pub fn yaml_token_delete(token: &mut yaml_token_t) {
    *token = yaml_token_t::default();
}

/// Create the STREAM-START event.
pub fn yaml_stream_start_event_initialize(
    event: &mut yaml_event_t,
    encoding: yaml_encoding_t,
) -> Result<(), ()> {
    *event = yaml_event_t {
        data: YamlEventData::StreamStart { encoding },
        ..Default::default()
    };
    Ok(())
}

/// Create the STREAM-END event.
pub fn yaml_stream_end_event_initialize(event: &mut yaml_event_t) -> Result<(), ()> {
    *event = yaml_event_t {
        data: YamlEventData::StreamEnd,
        ..Default::default()
    };
    Ok(())
}

/// Create the DOCUMENT-START event.
///
/// The `implicit` argument is considered as a stylistic parameter and may be
/// ignored by the emitter.
pub fn yaml_document_start_event_initialize(
    event: &mut yaml_event_t,
    version_directive: Option<yaml_version_directive_t>,
    tag_directives_in: &[yaml_tag_directive_t],
    implicit: bool,
) -> Result<(), ()> {
    let tag_directives = Vec::from_iter(tag_directives_in.iter().cloned());

    *event = yaml_event_t {
        data: YamlEventData::DocumentStart {
            version_directive,
            tag_directives,
            implicit,
        },
        ..Default::default()
    };

    Ok(())
}

/// Create the DOCUMENT-END event.
///
/// The `implicit` argument is considered as a stylistic parameter and may be
/// ignored by the emitter.
pub fn yaml_document_end_event_initialize(
    event: &mut yaml_event_t,
    implicit: bool,
) -> Result<(), ()> {
    *event = yaml_event_t {
        data: YamlEventData::DocumentEnd { implicit },
        ..Default::default()
    };
    Ok(())
}

/// Create an ALIAS event.
pub fn yaml_alias_event_initialize(event: &mut yaml_event_t, anchor: &str) -> Result<(), ()> {
    *event = yaml_event_t {
        data: YamlEventData::Alias {
            anchor: String::from(anchor),
        },
        ..Default::default()
    };
    Ok(())
}

/// Create a SCALAR event.
///
/// The `style` argument may be ignored by the emitter.
///
/// Either the `tag` attribute or one of the `plain_implicit` and
/// `quoted_implicit` flags must be set.
///
pub fn yaml_scalar_event_initialize(
    event: &mut yaml_event_t,
    anchor: Option<&str>,
    tag: Option<&str>,
    value: &str,
    plain_implicit: bool,
    quoted_implicit: bool,
    style: yaml_scalar_style_t,
) -> Result<(), ()> {
    let mark = yaml_mark_t {
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

    *event = yaml_event_t {
        data: YamlEventData::Scalar {
            anchor: anchor_copy,
            tag: tag_copy,
            value: String::from(value),
            plain_implicit,
            quoted_implicit,
            style,
        },
        start_mark: mark,
        end_mark: mark,
    };
    Ok(())
}

/// Create a SEQUENCE-START event.
///
/// The `style` argument may be ignored by the emitter.
///
/// Either the `tag` attribute or the `implicit` flag must be set.
pub fn yaml_sequence_start_event_initialize(
    event: &mut yaml_event_t,
    anchor: Option<&str>,
    tag: Option<&str>,
    implicit: bool,
    style: yaml_sequence_style_t,
) -> Result<(), ()> {
    let mut anchor_copy: Option<String> = None;
    let mut tag_copy: Option<String> = None;

    if let Some(anchor) = anchor {
        anchor_copy = Some(String::from(anchor));
    }
    if let Some(tag) = tag {
        tag_copy = Some(String::from(tag));
    }

    *event = yaml_event_t {
        data: YamlEventData::SequenceStart {
            anchor: anchor_copy,
            tag: tag_copy,
            implicit,
            style,
        },
        ..Default::default()
    };
    return Ok(());
}

/// Create a SEQUENCE-END event.
pub fn yaml_sequence_end_event_initialize(event: &mut yaml_event_t) -> Result<(), ()> {
    *event = yaml_event_t {
        data: YamlEventData::SequenceEnd,
        ..Default::default()
    };
    Ok(())
}

/// Create a MAPPING-START event.
///
/// The `style` argument may be ignored by the emitter.
///
/// Either the `tag` attribute or the `implicit` flag must be set.
pub fn yaml_mapping_start_event_initialize(
    event: &mut yaml_event_t,
    anchor: Option<&str>,
    tag: Option<&str>,
    implicit: bool,
    style: yaml_mapping_style_t,
) -> Result<(), ()> {
    let mut anchor_copy: Option<String> = None;
    let mut tag_copy: Option<String> = None;

    if let Some(anchor) = anchor {
        anchor_copy = Some(String::from(anchor));
    }

    if let Some(tag) = tag {
        tag_copy = Some(String::from(tag));
    }

    *event = yaml_event_t {
        data: YamlEventData::MappingStart {
            anchor: anchor_copy,
            tag: tag_copy,
            implicit,
            style,
        },
        ..Default::default()
    };

    Ok(())
}

/// Create a MAPPING-END event.
pub fn yaml_mapping_end_event_initialize(event: &mut yaml_event_t) -> Result<(), ()> {
    *event = yaml_event_t {
        data: YamlEventData::MappingEnd,
        ..Default::default()
    };
    Ok(())
}

/// Free any memory allocated for an event object.
pub fn yaml_event_delete(event: &mut yaml_event_t) {
    *event = Default::default();
}

/// Create a YAML document.
pub fn yaml_document_initialize(
    document: &mut yaml_document_t,
    version_directive: Option<yaml_version_directive_t>,
    tag_directives_in: &[yaml_tag_directive_t],
    start_implicit: bool,
    end_implicit: bool,
) -> Result<(), ()> {
    let nodes = Vec::with_capacity(16);
    let tag_directives = Vec::from_iter(tag_directives_in.iter().cloned());

    *document = yaml_document_t {
        nodes,
        version_directive,
        tag_directives,
        start_implicit,
        end_implicit,
        ..Default::default()
    };

    return Ok(());
}

/// Delete a YAML document and all its nodes.
pub fn yaml_document_delete(document: &mut yaml_document_t) {
    document.nodes.clear();
    document.version_directive = None;
    document.tag_directives.clear();
}

/// Get a node of a YAML document.
///
/// The pointer returned by this function is valid until any of the functions
/// modifying the documents are called.
///
/// Returns the node object or NULL if `index` is out of range.
pub fn yaml_document_get_node(
    document: &mut yaml_document_t,
    index: libc::c_int,
) -> *mut yaml_node_t {
    if index > 0 && index as usize <= document.nodes.len() {
        return &mut document.nodes[index as usize - 1] as *mut _;
    }
    ptr::null_mut()
}

/// Get the root of a YAML document node.
///
/// The root object is the first object added to the document.
///
/// The pointer returned by this function is valid until any of the functions
/// modifying the documents are called.
///
/// An empty document produced by the parser signifies the end of a YAML stream.
///
/// Returns the node object or NULL if the document is empty.
pub fn yaml_document_get_root_node(document: &mut yaml_document_t) -> *mut yaml_node_t {
    if let Some(root) = document.nodes.get_mut(0) {
        root as _
    } else {
        ptr::null_mut()
    }
}

/// Create a SCALAR node and attach it to the document.
///
/// The `style` argument may be ignored by the emitter.
///
/// Returns the node id or 0 on error.
#[must_use]
pub fn yaml_document_add_scalar(
    document: &mut yaml_document_t,
    tag: Option<&str>,
    value: &str,
    style: yaml_scalar_style_t,
) -> libc::c_int {
    let mark = yaml_mark_t {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };
    let tag = tag.unwrap_or("tag:yaml.org,2002:str");
    let tag_copy = String::from(tag);
    let value_copy = String::from(value);
    let node = yaml_node_t {
        data: YamlNodeData::Scalar {
            value: value_copy,
            style,
        },
        tag: Some(tag_copy),
        start_mark: mark,
        end_mark: mark,
    };
    document.nodes.push(node);
    document.nodes.len() as libc::c_int
}

/// Create a SEQUENCE node and attach it to the document.
///
/// The `style` argument may be ignored by the emitter.
///
/// Returns the node id or 0 on error.
#[must_use]
pub fn yaml_document_add_sequence(
    document: &mut yaml_document_t,
    tag: Option<&str>,
    style: yaml_sequence_style_t,
) -> libc::c_int {
    let mark = yaml_mark_t {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };

    let items = Vec::with_capacity(16);
    let tag = tag.unwrap_or("tag:yaml.org,2002:seq");
    let tag_copy = String::from(tag);
    let node = yaml_node_t {
        data: YamlNodeData::Sequence { items, style },
        tag: Some(tag_copy),
        start_mark: mark,
        end_mark: mark,
    };
    document.nodes.push(node);
    document.nodes.len() as libc::c_int
}

/// Create a MAPPING node and attach it to the document.
///
/// The `style` argument may be ignored by the emitter.
///
/// Returns the node id or 0 on error.
#[must_use]
pub fn yaml_document_add_mapping(
    document: &mut yaml_document_t,
    tag: Option<&str>,
    style: yaml_mapping_style_t,
) -> libc::c_int {
    let mark = yaml_mark_t {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };
    let pairs = Vec::with_capacity(16);
    let tag = tag.unwrap_or("tag:yaml.org,2002:map");
    let tag_copy = String::from(tag);

    let node = yaml_node_t {
        data: YamlNodeData::Mapping { pairs, style },
        tag: Some(tag_copy),
        start_mark: mark,
        end_mark: mark,
    };

    document.nodes.push(node);
    document.nodes.len() as libc::c_int
}

/// Add an item to a SEQUENCE node.
pub fn yaml_document_append_sequence_item(
    document: &mut yaml_document_t,
    sequence: libc::c_int,
    item: libc::c_int,
) -> Result<(), ()> {
    __assert!(sequence > 0 && sequence as usize - 1 < document.nodes.len());
    __assert!(matches!(
        &document.nodes[sequence as usize - 1].data,
        YamlNodeData::Sequence { .. }
    ));
    __assert!(item > 0 && item as usize - 1 < document.nodes.len());
    if let YamlNodeData::Sequence { ref mut items, .. } =
        &mut document.nodes[sequence as usize - 1].data
    {
        items.push(item);
    }
    Ok(())
}

/// Add a pair of a key and a value to a MAPPING node.
pub fn yaml_document_append_mapping_pair(
    document: &mut yaml_document_t,
    mapping: libc::c_int,
    key: libc::c_int,
    value: libc::c_int,
) -> Result<(), ()> {
    __assert!(mapping > 0 && mapping as usize - 1 < document.nodes.len());
    __assert!(matches!(
        &document.nodes[mapping as usize - 1].data,
        YamlNodeData::Mapping { .. }
    ));
    __assert!(key > 0 && key as usize - 1 < document.nodes.len());
    __assert!(value > 0 && value as usize - 1 < document.nodes.len());
    let pair = yaml_node_pair_t { key, value };
    if let YamlNodeData::Mapping { ref mut pairs, .. } =
        &mut document.nodes[mapping as usize - 1].data
    {
        pairs.push(pair);
    }
    Ok(())
}
