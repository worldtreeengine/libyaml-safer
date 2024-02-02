use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{api::yaml_parser_new, libc, yaml_emitter_new};
use core::ptr;

pub use self::yaml_encoding_t::*;
pub use core::primitive::{i64 as ptrdiff_t, u64 as size_t};

/// The version directive data.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
#[non_exhaustive]
pub struct yaml_version_directive_t {
    /// The major version number.
    pub major: libc::c_int,
    /// The minor version number.
    pub minor: libc::c_int,
}

/// The tag directive data.
#[derive(Debug, Clone)]
#[repr(C)]
#[non_exhaustive]
pub struct yaml_tag_directive_t {
    /// The tag handle.
    pub handle: String,
    /// The tag prefix.
    pub prefix: String,
}

/// The stream encoding.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[non_exhaustive]
pub enum yaml_encoding_t {
    /// Let the parser choose the encoding.
    #[default]
    YAML_ANY_ENCODING = 0,
    /// The default UTF-8 encoding.
    YAML_UTF8_ENCODING = 1,
    /// The UTF-16-LE encoding with BOM.
    YAML_UTF16LE_ENCODING = 2,
    /// The UTF-16-BE encoding with BOM.
    YAML_UTF16BE_ENCODING = 3,
}

/// Line break type.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[non_exhaustive]
pub enum yaml_break_t {
    /// Let the parser choose the break type.
    #[default]
    YAML_ANY_BREAK = 0,
    /// Use CR for line breaks (Mac style).
    YAML_CR_BREAK = 1,
    /// Use LN for line breaks (Unix style).
    YAML_LN_BREAK = 2,
    /// Use CR LN for line breaks (DOS style).
    YAML_CRLN_BREAK = 3,
}

/// The pointer position.
#[derive(Copy, Clone, Default, Debug)]
#[repr(C)]
#[non_exhaustive]
pub struct yaml_mark_t {
    /// The position index.
    pub index: size_t,
    /// The position line.
    pub line: size_t,
    /// The position column.
    pub column: size_t,
}

/// Scalar styles.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[non_exhaustive]
pub enum yaml_scalar_style_t {
    /// Let the emitter choose the style.
    #[default]
    YAML_ANY_SCALAR_STYLE = 0,
    /// The plain scalar style.
    YAML_PLAIN_SCALAR_STYLE = 1,
    /// The single-quoted scalar style.
    YAML_SINGLE_QUOTED_SCALAR_STYLE = 2,
    /// The double-quoted scalar style.
    YAML_DOUBLE_QUOTED_SCALAR_STYLE = 3,
    /// The literal scalar style.
    YAML_LITERAL_SCALAR_STYLE = 4,
    /// The folded scalar style.
    YAML_FOLDED_SCALAR_STYLE = 5,
}

/// Sequence styles.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[non_exhaustive]
pub enum yaml_sequence_style_t {
    /// Let the emitter choose the style.
    YAML_ANY_SEQUENCE_STYLE = 0,
    /// The block sequence style.
    YAML_BLOCK_SEQUENCE_STYLE = 1,
    /// The flow sequence style.
    YAML_FLOW_SEQUENCE_STYLE = 2,
}

/// Mapping styles.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[non_exhaustive]
pub enum yaml_mapping_style_t {
    /// Let the emitter choose the style.
    YAML_ANY_MAPPING_STYLE = 0,
    /// The block mapping style.
    YAML_BLOCK_MAPPING_STYLE = 1,
    /// The flow mapping style.
    YAML_FLOW_MAPPING_STYLE = 2,
}

/// Token types.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[non_exhaustive]
pub enum yaml_token_type_t {
    /// An empty token.
    YAML_NO_TOKEN = 0,
    /// A STREAM-START token.
    YAML_STREAM_START_TOKEN = 1,
    /// A STREAM-END token.
    YAML_STREAM_END_TOKEN = 2,
    /// A VERSION-DIRECTIVE token.
    YAML_VERSION_DIRECTIVE_TOKEN = 3,
    /// A TAG-DIRECTIVE token.
    YAML_TAG_DIRECTIVE_TOKEN = 4,
    /// A DOCUMENT-START token.
    YAML_DOCUMENT_START_TOKEN = 5,
    /// A DOCUMENT-END token.
    YAML_DOCUMENT_END_TOKEN = 6,
    /// A BLOCK-SEQUENCE-START token.
    YAML_BLOCK_SEQUENCE_START_TOKEN = 7,
    /// A BLOCK-MAPPING-START token.
    YAML_BLOCK_MAPPING_START_TOKEN = 8,
    /// A BLOCK-END token.
    YAML_BLOCK_END_TOKEN = 9,
    /// A FLOW-SEQUENCE-START token.
    YAML_FLOW_SEQUENCE_START_TOKEN = 10,
    /// A FLOW-SEQUENCE-END token.
    YAML_FLOW_SEQUENCE_END_TOKEN = 11,
    /// A FLOW-MAPPING-START token.
    YAML_FLOW_MAPPING_START_TOKEN = 12,
    /// A FLOW-MAPPING-END token.
    YAML_FLOW_MAPPING_END_TOKEN = 13,
    /// A BLOCK-ENTRY token.
    YAML_BLOCK_ENTRY_TOKEN = 14,
    /// A FLOW-ENTRY token.
    YAML_FLOW_ENTRY_TOKEN = 15,
    /// A KEY token.
    YAML_KEY_TOKEN = 16,
    /// A VALUE token.
    YAML_VALUE_TOKEN = 17,
    /// An ALIAS token.
    YAML_ALIAS_TOKEN = 18,
    /// An ANCHOR token.
    YAML_ANCHOR_TOKEN = 19,
    /// A TAG token.
    YAML_TAG_TOKEN = 20,
    /// A SCALAR token.
    YAML_SCALAR_TOKEN = 21,
}

/// The token structure.
#[derive(Default)]
#[repr(C)]
#[non_exhaustive]
pub struct yaml_token_t {
    /// The token type.
    pub data: YamlTokenData,
    /// The beginning of the token.
    pub start_mark: yaml_mark_t,
    /// The end of the token.
    pub end_mark: yaml_mark_t,
}

#[derive(Default)]
pub enum YamlTokenData {
    /// An empty token.
    #[default]
    NoToken,
    /// A STREAM-START token.
    StreamStart {
        /// The stream encoding.
        encoding: yaml_encoding_t,
    },
    /// A STREAM-END token.
    StreamEnd,
    /// A VERSION-DIRECTIVE token.
    VersionDirective {
        /// The major version number.
        major: libc::c_int,
        /// The minor version number.
        minor: libc::c_int,
    },
    /// A TAG-DIRECTIVE token.
    TagDirective {
        /// The tag handle.
        handle: String,
        /// The tag prefix.
        prefix: String,
    },
    /// A DOCUMENT-START token.
    DocumentStart,
    /// A DOCUMENT-END token.
    DocumentEnd,
    /// A BLOCK-SEQUENCE-START token.
    BlockSequenceStart,
    /// A BLOCK-MAPPING-START token.
    BlockMappingStart,
    /// A BLOCK-END token.
    BlockEnd,
    /// A FLOW-SEQUENCE-START token.
    FlowSequenceStart,
    /// A FLOW-SEQUENCE-END token.
    FlowSequenceEnd,
    /// A FLOW-MAPPING-START token.
    FlowMappingStart,
    /// A FLOW-MAPPING-END token.
    FlowMappingEnd,
    /// A BLOCK-ENTRY token.
    BlockEntry,
    /// A FLOW-ENTRY token.
    FlowEntry,
    /// A KEY token.
    Key,
    /// A VALUE token.
    Value,
    /// An ALIAS token.
    Alias {
        /// The alias value.
        value: String,
    },
    /// An ANCHOR token.
    Anchor {
        /// The anchor value.
        value: String,
    },
    /// A TAG token.
    Tag {
        /// The tag handle.
        handle: String,
        /// The tag suffix.
        suffix: String,
    },
    /// A SCALAR token.
    Scalar {
        /// The scalar value.
        value: String,
        /// The scalar style.
        style: yaml_scalar_style_t,
    },
}

impl YamlTokenData {
    /// Returns `true` if the yaml token data is [`VersionDirective`].
    ///
    /// [`VersionDirective`]: YamlTokenData::VersionDirective
    #[must_use]
    pub fn is_version_directive(&self) -> bool {
        matches!(self, Self::VersionDirective { .. })
    }

    /// Returns `true` if the yaml token data is [`TagDirective`].
    ///
    /// [`TagDirective`]: YamlTokenData::TagDirective
    #[must_use]
    pub fn is_tag_directive(&self) -> bool {
        matches!(self, Self::TagDirective { .. })
    }

    /// Returns `true` if the yaml token data is [`DocumentStart`].
    ///
    /// [`DocumentStart`]: YamlTokenData::DocumentStart
    #[must_use]
    pub fn is_document_start(&self) -> bool {
        matches!(self, Self::DocumentStart)
    }

    /// Returns `true` if the yaml token data is [`StreamEnd`].
    ///
    /// [`StreamEnd`]: YamlTokenData::StreamEnd
    #[must_use]
    pub fn is_stream_end(&self) -> bool {
        matches!(self, Self::StreamEnd)
    }

    /// Returns `true` if the yaml token data is [`BlockEntry`].
    ///
    /// [`BlockEntry`]: YamlTokenData::BlockEntry
    #[must_use]
    pub fn is_block_entry(&self) -> bool {
        matches!(self, Self::BlockEntry)
    }

    /// Returns `true` if the yaml token data is [`BlockSequenceStart`].
    ///
    /// [`BlockSequenceStart`]: YamlTokenData::BlockSequenceStart
    #[must_use]
    pub fn is_block_sequence_start(&self) -> bool {
        matches!(self, Self::BlockSequenceStart)
    }

    /// Returns `true` if the yaml token data is [`BlockMappingStart`].
    ///
    /// [`BlockMappingStart`]: YamlTokenData::BlockMappingStart
    #[must_use]
    pub fn is_block_mapping_start(&self) -> bool {
        matches!(self, Self::BlockMappingStart)
    }

    /// Returns `true` if the yaml token data is [`BlockEnd`].
    ///
    /// [`BlockEnd`]: YamlTokenData::BlockEnd
    #[must_use]
    pub fn is_block_end(&self) -> bool {
        matches!(self, Self::BlockEnd)
    }

    /// Returns `true` if the yaml token data is [`Key`].
    ///
    /// [`Key`]: YamlTokenData::Key
    #[must_use]
    pub fn is_key(&self) -> bool {
        matches!(self, Self::Key)
    }

    /// Returns `true` if the yaml token data is [`Value`].
    ///
    /// [`Value`]: YamlTokenData::Value
    #[must_use]
    pub fn is_value(&self) -> bool {
        matches!(self, Self::Value)
    }

    /// Returns `true` if the yaml token data is [`FlowSequenceEnd`].
    ///
    /// [`FlowSequenceEnd`]: YamlTokenData::FlowSequenceEnd
    #[must_use]
    pub fn is_flow_sequence_end(&self) -> bool {
        matches!(self, Self::FlowSequenceEnd)
    }

    /// Returns `true` if the yaml token data is [`FlowEntry`].
    ///
    /// [`FlowEntry`]: YamlTokenData::FlowEntry
    #[must_use]
    pub fn is_flow_entry(&self) -> bool {
        matches!(self, Self::FlowEntry)
    }

    /// Returns `true` if the yaml token data is [`FlowMappingEnd`].
    ///
    /// [`FlowMappingEnd`]: YamlTokenData::FlowMappingEnd
    #[must_use]
    pub fn is_flow_mapping_end(&self) -> bool {
        matches!(self, Self::FlowMappingEnd)
    }
}

/// The event structure.
#[derive(Default, Debug)]
#[repr(C)]
#[non_exhaustive]
pub struct yaml_event_t {
    /// The event data.
    pub data: YamlEventData,
    /// The beginning of the event.
    pub start_mark: yaml_mark_t,
    /// The end of the event.
    pub end_mark: yaml_mark_t,
}

#[derive(Default, Debug)]
pub enum YamlEventData {
    #[default]
    NoEvent,
    /// The stream parameters (for YAML_STREAM_START_EVENT).
    StreamStart {
        /// The document encoding.
        encoding: yaml_encoding_t,
    },
    StreamEnd,
    /// The document parameters (for YAML_DOCUMENT_START_EVENT).
    DocumentStart {
        /// The version directive.
        version_directive: Option<yaml_version_directive_t>,
        /// The tag directives list.
        tag_directives: Vec<yaml_tag_directive_t>,
        /// Is the document indicator implicit?
        implicit: bool,
    },
    /// The document end parameters (for YAML_DOCUMENT_END_EVENT).
    DocumentEnd {
        implicit: bool,
    },
    /// The alias parameters (for YAML_ALIAS_EVENT).
    Alias {
        /// The anchor.
        anchor: String,
    },
    /// The scalar parameters (for YAML_SCALAR_EVENT).
    Scalar {
        /// The anchor.
        anchor: Option<String>,
        /// The tag.
        tag: Option<String>,
        /// The scalar value.
        value: String,
        /// Is the tag optional for the plain style?
        plain_implicit: bool,
        /// Is the tag optional for any non-plain style?
        quoted_implicit: bool,
        /// The scalar style.
        style: yaml_scalar_style_t,
    },
    /// The sequence parameters (for YAML_SEQUENCE_START_EVENT).
    SequenceStart {
        /// The anchor.
        anchor: Option<String>,
        /// The tag.
        tag: Option<String>,
        /// Is the tag optional?
        implicit: bool,
        /// The sequence style.
        style: yaml_sequence_style_t,
    },
    SequenceEnd,
    /// The mapping parameters (for YAML_MAPPING_START_EVENT).
    MappingStart {
        /// The anchor.
        anchor: Option<String>,
        /// The tag.
        tag: Option<String>,
        /// Is the tag optional?
        implicit: bool,
        /// The mapping style.
        style: yaml_mapping_style_t,
    },
    MappingEnd,
}

/// The node structure.
#[derive(Default)]
#[repr(C)]
#[non_exhaustive]
pub struct yaml_node_t {
    /// The node type.
    pub data: YamlNodeData,
    /// The node tag.
    pub tag: Option<String>,
    /// The beginning of the node.
    pub start_mark: yaml_mark_t,
    /// The end of the node.
    pub end_mark: yaml_mark_t,
}

/// Node types.
#[derive(Default)]
pub enum YamlNodeData {
    /// An empty node.
    #[default]
    NoNode,
    /// A scalar node.
    Scalar {
        /// The scalar value.
        value: String,
        /// The scalar style.
        style: yaml_scalar_style_t,
    },
    /// A sequence node.
    Sequence {
        /// The stack of sequence items.
        items: Vec<yaml_node_item_t>,
        /// The sequence style.
        style: yaml_sequence_style_t,
    },
    /// A mapping node.
    Mapping {
        /// The stack of mapping pairs (key, value).
        pairs: Vec<yaml_node_pair_t>,
        /// The mapping style.
        style: yaml_mapping_style_t,
    },
}

/// An element of a sequence node.
pub type yaml_node_item_t = libc::c_int;
/// An element of a mapping node.
#[derive(Copy, Clone, Default)]
#[repr(C)]
#[non_exhaustive]
pub struct yaml_node_pair_t {
    /// The key of the element.
    pub key: libc::c_int,
    /// The value of the element.
    pub value: libc::c_int,
}

/// The document structure.
#[repr(C)]
#[non_exhaustive]
pub struct yaml_document_t {
    /// The document nodes.
    pub nodes: Vec<yaml_node_t>,
    /// The version directive.
    pub version_directive: Option<yaml_version_directive_t>,
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
    pub tag_directives: Vec<yaml_tag_directive_t>,
    /// Is the document start indicator implicit?
    pub start_implicit: bool,
    /// Is the document end indicator implicit?
    pub end_implicit: bool,
    /// The beginning of the document.
    pub start_mark: yaml_mark_t,
    /// The end of the document.
    pub end_mark: yaml_mark_t,
}

/// This structure holds information about a potential simple key.
#[derive(Copy, Clone)]
#[repr(C)]
#[non_exhaustive]
pub struct yaml_simple_key_t {
    /// Is a simple key possible?
    pub possible: bool,
    /// Is a simple key required?
    pub required: bool,
    /// The number of the token.
    pub token_number: size_t,
    /// The position mark.
    pub mark: yaml_mark_t,
}

/// The states of the parser.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[non_exhaustive]
pub enum yaml_parser_state_t {
    /// Expect STREAM-START.
    #[default]
    YAML_PARSE_STREAM_START_STATE = 0,
    /// Expect the beginning of an implicit document.
    YAML_PARSE_IMPLICIT_DOCUMENT_START_STATE = 1,
    /// Expect DOCUMENT-START.
    YAML_PARSE_DOCUMENT_START_STATE = 2,
    /// Expect the content of a document.
    YAML_PARSE_DOCUMENT_CONTENT_STATE = 3,
    /// Expect DOCUMENT-END.
    YAML_PARSE_DOCUMENT_END_STATE = 4,
    /// Expect a block node.
    YAML_PARSE_BLOCK_NODE_STATE = 5,
    /// Expect a block node or indentless sequence.
    YAML_PARSE_BLOCK_NODE_OR_INDENTLESS_SEQUENCE_STATE = 6,
    /// Expect a flow node.
    YAML_PARSE_FLOW_NODE_STATE = 7,
    /// Expect the first entry of a block sequence.
    YAML_PARSE_BLOCK_SEQUENCE_FIRST_ENTRY_STATE = 8,
    /// Expect an entry of a block sequence.
    YAML_PARSE_BLOCK_SEQUENCE_ENTRY_STATE = 9,
    /// Expect an entry of an indentless sequence.
    YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE = 10,
    /// Expect the first key of a block mapping.
    YAML_PARSE_BLOCK_MAPPING_FIRST_KEY_STATE = 11,
    /// Expect a block mapping key.
    YAML_PARSE_BLOCK_MAPPING_KEY_STATE = 12,
    /// Expect a block mapping value.
    YAML_PARSE_BLOCK_MAPPING_VALUE_STATE = 13,
    /// Expect the first entry of a flow sequence.
    YAML_PARSE_FLOW_SEQUENCE_FIRST_ENTRY_STATE = 14,
    /// Expect an entry of a flow sequence.
    YAML_PARSE_FLOW_SEQUENCE_ENTRY_STATE = 15,
    /// Expect a key of an ordered mapping.
    YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_KEY_STATE = 16,
    /// Expect a value of an ordered mapping.
    YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_VALUE_STATE = 17,
    /// Expect the and of an ordered mapping entry.
    YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_END_STATE = 18,
    /// Expect the first key of a flow mapping.
    YAML_PARSE_FLOW_MAPPING_FIRST_KEY_STATE = 19,
    /// Expect a key of a flow mapping.
    YAML_PARSE_FLOW_MAPPING_KEY_STATE = 20,
    /// Expect a value of a flow mapping.
    YAML_PARSE_FLOW_MAPPING_VALUE_STATE = 21,
    /// Expect an empty value of a flow mapping.
    YAML_PARSE_FLOW_MAPPING_EMPTY_VALUE_STATE = 22,
    /// Expect nothing.
    YAML_PARSE_END_STATE = 23,
}

/// This structure holds aliases data.
#[repr(C)]
#[non_exhaustive]
pub struct yaml_alias_data_t {
    /// The anchor.
    pub anchor: String,
    /// The node id.
    pub index: libc::c_int,
    /// The anchor mark.
    pub mark: yaml_mark_t,
}

/// The parser structure.
///
/// All members are internal. Manage the structure using the `yaml_parser_`
/// family of functions.
#[repr(C)]
#[non_exhaustive]
pub struct yaml_parser_t<'r> {
    /// Read handler.
    pub(crate) read_handler: Option<&'r mut dyn std::io::Read>,
    /// Standard (string or file) input data.
    pub(crate) input: unnamed_yaml_parser_t_input_string,
    /// EOF flag
    pub(crate) eof: bool,
    /// The working buffer.
    ///
    /// This always contains valid UTF-8.
    pub(crate) buffer: VecDeque<char>,
    /// The number of unread characters in the buffer.
    pub(crate) unread: size_t,
    /// The raw buffer.
    ///
    /// This is the raw unchecked input from the read handler (for example, it
    /// may be UTF-16 encoded).
    // TODO: Get rid of this and ask users to provide something implementing `BufRead` instead of `Read`.
    pub(crate) raw_buffer: VecDeque<u8>,
    /// The input encoding.
    pub(crate) encoding: yaml_encoding_t,
    /// The offset of the current position (in bytes).
    pub(crate) offset: size_t,
    /// The mark of the current position.
    pub(crate) mark: yaml_mark_t,
    /// Have we started to scan the input stream?
    pub(crate) stream_start_produced: bool,
    /// Have we reached the end of the input stream?
    pub(crate) stream_end_produced: bool,
    /// The number of unclosed '[' and '{' indicators.
    pub(crate) flow_level: libc::c_int,
    /// The tokens queue.
    pub(crate) tokens: VecDeque<yaml_token_t>,
    /// The number of tokens fetched from the queue.
    pub(crate) tokens_parsed: size_t,
    /// Does the tokens queue contain a token ready for dequeueing.
    pub(crate) token_available: bool,
    /// The indentation levels stack.
    pub(crate) indents: Vec<libc::c_int>,
    /// The current indentation level.
    pub(crate) indent: libc::c_int,
    /// May a simple key occur at the current position?
    pub(crate) simple_key_allowed: bool,
    /// The stack of simple keys.
    pub(crate) simple_keys: Vec<yaml_simple_key_t>,
    /// The parser states stack.
    pub(crate) states: Vec<yaml_parser_state_t>,
    /// The current parser state.
    pub(crate) state: yaml_parser_state_t,
    /// The stack of marks.
    pub(crate) marks: Vec<yaml_mark_t>,
    /// The list of TAG directives.
    pub(crate) tag_directives: Vec<yaml_tag_directive_t>,
    /// The alias data.
    pub(crate) aliases: Vec<yaml_alias_data_t>,
}

impl<'r> Default for yaml_parser_t<'r> {
    fn default() -> Self {
        yaml_parser_new()
    }
}

#[repr(C)]
pub(crate) struct unnamed_yaml_parser_t_input_string {
    /// The string start pointer.
    pub start: *const libc::c_uchar,
    /// The string end pointer.
    pub end: *const libc::c_uchar,
    /// The string current position.
    pub current: *const libc::c_uchar,
}

impl Default for unnamed_yaml_parser_t_input_string {
    fn default() -> Self {
        Self {
            start: ptr::null(),
            end: ptr::null(),
            current: ptr::null(),
        }
    }
}

/// The prototype of a write handler.
///
/// The write handler is called when the emitter needs to flush the accumulated
/// characters to the output. The handler should write `size` bytes of the
/// `buffer` to the output.
///
/// On success, the handler should return 1. If the handler failed, the returned
/// value should be 0.
pub type yaml_write_handler_t = fn(data: *mut libc::c_void, buffer: &[u8]) -> libc::c_int;

/// The emitter states.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u32)]
#[non_exhaustive]
pub enum yaml_emitter_state_t {
    /// Expect STREAM-START.
    #[default]
    YAML_EMIT_STREAM_START_STATE = 0,
    /// Expect the first DOCUMENT-START or STREAM-END.
    YAML_EMIT_FIRST_DOCUMENT_START_STATE = 1,
    /// Expect DOCUMENT-START or STREAM-END.
    YAML_EMIT_DOCUMENT_START_STATE = 2,
    /// Expect the content of a document.
    YAML_EMIT_DOCUMENT_CONTENT_STATE = 3,
    /// Expect DOCUMENT-END.
    YAML_EMIT_DOCUMENT_END_STATE = 4,
    /// Expect the first item of a flow sequence.
    YAML_EMIT_FLOW_SEQUENCE_FIRST_ITEM_STATE = 5,
    /// Expect an item of a flow sequence.
    YAML_EMIT_FLOW_SEQUENCE_ITEM_STATE = 6,
    /// Expect the first key of a flow mapping.
    YAML_EMIT_FLOW_MAPPING_FIRST_KEY_STATE = 7,
    /// Expect a key of a flow mapping.
    YAML_EMIT_FLOW_MAPPING_KEY_STATE = 8,
    /// Expect a value for a simple key of a flow mapping.
    YAML_EMIT_FLOW_MAPPING_SIMPLE_VALUE_STATE = 9,
    /// Expect a value of a flow mapping.
    YAML_EMIT_FLOW_MAPPING_VALUE_STATE = 10,
    /// Expect the first item of a block sequence.
    YAML_EMIT_BLOCK_SEQUENCE_FIRST_ITEM_STATE = 11,
    /// Expect an item of a block sequence.
    YAML_EMIT_BLOCK_SEQUENCE_ITEM_STATE = 12,
    /// Expect the first key of a block mapping.
    YAML_EMIT_BLOCK_MAPPING_FIRST_KEY_STATE = 13,
    /// Expect the key of a block mapping.
    YAML_EMIT_BLOCK_MAPPING_KEY_STATE = 14,
    /// Expect a value for a simple key of a block mapping.
    YAML_EMIT_BLOCK_MAPPING_SIMPLE_VALUE_STATE = 15,
    /// Expect a value of a block mapping.
    YAML_EMIT_BLOCK_MAPPING_VALUE_STATE = 16,
    /// Expect nothing.
    YAML_EMIT_END_STATE = 17,
}

#[derive(Copy, Clone, Default)]
#[repr(C)]
pub(crate) struct yaml_anchors_t {
    /// The number of references.
    pub references: libc::c_int,
    /// The anchor id.
    pub anchor: libc::c_int,
    /// If the node has been emitted?
    pub serialized: bool,
}

/// The emitter structure.
///
/// All members are internal. Manage the structure using the `yaml_emitter_`
/// family of functions.
#[repr(C)]
#[non_exhaustive]
pub struct yaml_emitter_t<'w> {
    /// Write handler.
    pub(crate) write_handler: Option<&'w mut dyn std::io::Write>,
    /// Standard (string or file) output data.
    pub(crate) output: unnamed_yaml_emitter_t_output_string,
    /// The working buffer.
    ///
    /// This always contains valid UTF-8.
    pub(crate) buffer: String,
    /// The raw buffer.
    ///
    /// This contains the output in the encoded format, so for example it may be
    /// UTF-16 encoded.
    pub(crate) raw_buffer: Vec<u8>,
    /// The stream encoding.
    pub(crate) encoding: yaml_encoding_t,
    /// If the output is in the canonical style?
    pub(crate) canonical: bool,
    /// The number of indentation spaces.
    pub(crate) best_indent: libc::c_int,
    /// The preferred width of the output lines.
    pub(crate) best_width: libc::c_int,
    /// Allow unescaped non-ASCII characters?
    pub(crate) unicode: bool,
    /// The preferred line break.
    pub(crate) line_break: yaml_break_t,
    /// The stack of states.
    pub(crate) states: Vec<yaml_emitter_state_t>,
    /// The current emitter state.
    pub(crate) state: yaml_emitter_state_t,
    /// The event queue.
    pub(crate) events: VecDeque<yaml_event_t>,
    /// The stack of indentation levels.
    pub(crate) indents: Vec<libc::c_int>,
    /// The list of tag directives.
    pub(crate) tag_directives: Vec<yaml_tag_directive_t>,
    /// The current indentation level.
    pub(crate) indent: libc::c_int,
    /// The current flow level.
    pub(crate) flow_level: libc::c_int,
    /// Is it the document root context?
    pub(crate) root_context: bool,
    /// Is it a sequence context?
    pub(crate) sequence_context: bool,
    /// Is it a mapping context?
    pub(crate) mapping_context: bool,
    /// Is it a simple mapping key context?
    pub(crate) simple_key_context: bool,
    /// The current line.
    pub(crate) line: libc::c_int,
    /// The current column.
    pub(crate) column: libc::c_int,
    /// If the last character was a whitespace?
    pub(crate) whitespace: bool,
    /// If the last character was an indentation character (' ', '-', '?', ':')?
    pub(crate) indention: bool,
    /// If an explicit document end is required?
    pub(crate) open_ended: libc::c_int,
    /// If the stream was already opened?
    pub(crate) opened: bool,
    /// If the stream was already closed?
    pub(crate) closed: bool,
    /// The information associated with the document nodes.
    // Note: Same length as `document.nodes`.
    pub(crate) anchors: Vec<yaml_anchors_t>,
    /// The last assigned anchor id.
    pub(crate) last_anchor_id: libc::c_int,
}

impl<'a> Default for yaml_emitter_t<'a> {
    fn default() -> Self {
        yaml_emitter_new()
    }
}

#[repr(C)]
pub(crate) struct unnamed_yaml_emitter_t_output_string {
    /// The buffer pointer.
    pub buffer: *mut libc::c_uchar,
    /// The buffer size.
    pub size: size_t,
    /// The number of written bytes.
    pub size_written: *mut size_t,
}

impl Default for unnamed_yaml_emitter_t_output_string {
    fn default() -> Self {
        Self {
            buffer: ptr::null_mut(),
            size: 0,
            size_written: ptr::null_mut(),
        }
    }
}
