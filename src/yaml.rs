use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{api::yaml_parser_new, yaml_emitter_new};

pub use self::Encoding::*;

/// The tag @c !!null with the only possible value: @c null.
pub const YAML_NULL_TAG: &str = "tag:yaml.org,2002:null";
/// The tag @c !!bool with the values: @c true and @c false.
pub const YAML_BOOL_TAG: &str = "tag:yaml.org,2002:bool";
/// The tag @c !!str for string values.
pub const YAML_STR_TAG: &str = "tag:yaml.org,2002:str";
/// The tag @c !!int for integer values.
pub const YAML_INT_TAG: &str = "tag:yaml.org,2002:int";
/// The tag @c !!float for float values.
pub const YAML_FLOAT_TAG: &str = "tag:yaml.org,2002:float";
/// The tag @c !!timestamp for date and time values.
pub const YAML_TIMESTAMP_TAG: &str = "tag:yaml.org,2002:timestamp";

/// The tag @c !!seq is used to denote sequences.
pub const YAML_SEQ_TAG: &str = "tag:yaml.org,2002:seq";
/// The tag @c !!map is used to denote mapping.
pub const YAML_MAP_TAG: &str = "tag:yaml.org,2002:map";

/// The default scalar tag is @c !!str.
pub const YAML_DEFAULT_SCALAR_TAG: &str = YAML_STR_TAG;
/// The default sequence tag is @c !!seq.
pub const YAML_DEFAULT_SEQUENCE_TAG: &str = YAML_SEQ_TAG;
/// The default mapping tag is @c !!map.
pub const YAML_DEFAULT_MAPPING_TAG: &str = YAML_MAP_TAG;

/// The version directive data.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct VersionDirective {
    /// The major version number.
    pub major: i32,
    /// The minor version number.
    pub minor: i32,
}

/// The tag directive data.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TagDirective {
    /// The tag handle.
    pub handle: String,
    /// The tag prefix.
    pub prefix: String,
}

/// The stream encoding.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum Encoding {
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
#[non_exhaustive]
pub enum Break {
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
#[non_exhaustive]
pub struct Mark {
    /// The position index.
    pub index: u64,
    /// The position line.
    pub line: u64,
    /// The position column.
    pub column: u64,
}

/// Scalar styles.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum ScalarStyle {
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
#[non_exhaustive]
pub enum SequenceStyle {
    /// Let the emitter choose the style.
    YAML_ANY_SEQUENCE_STYLE = 0,
    /// The block sequence style.
    YAML_BLOCK_SEQUENCE_STYLE = 1,
    /// The flow sequence style.
    YAML_FLOW_SEQUENCE_STYLE = 2,
}

/// Mapping styles.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum MappingStyle {
    /// Let the emitter choose the style.
    YAML_ANY_MAPPING_STYLE = 0,
    /// The block mapping style.
    YAML_BLOCK_MAPPING_STYLE = 1,
    /// The flow mapping style.
    YAML_FLOW_MAPPING_STYLE = 2,
}

/// The token structure.
#[derive(Default)]
#[non_exhaustive]
pub struct Token {
    /// The token type.
    pub data: TokenData,
    /// The beginning of the token.
    pub start_mark: Mark,
    /// The end of the token.
    pub end_mark: Mark,
}

#[derive(Default)]
pub enum TokenData {
    /// An empty token.
    #[default]
    NoToken,
    /// A STREAM-START token.
    StreamStart {
        /// The stream encoding.
        encoding: Encoding,
    },
    /// A STREAM-END token.
    StreamEnd,
    /// A VERSION-DIRECTIVE token.
    VersionDirective {
        /// The major version number.
        major: i32,
        /// The minor version number.
        minor: i32,
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
        style: ScalarStyle,
    },
}

impl TokenData {
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
#[non_exhaustive]
pub struct Event {
    /// The event data.
    pub data: EventData,
    /// The beginning of the event.
    pub start_mark: Mark,
    /// The end of the event.
    pub end_mark: Mark,
}

#[derive(Default, Debug)]
pub enum EventData {
    #[default]
    NoEvent,
    /// The stream parameters (for YAML_STREAM_START_EVENT).
    StreamStart {
        /// The document encoding.
        encoding: Encoding,
    },
    StreamEnd,
    /// The document parameters (for YAML_DOCUMENT_START_EVENT).
    DocumentStart {
        /// The version directive.
        version_directive: Option<VersionDirective>,
        /// The tag directives list.
        tag_directives: Vec<TagDirective>,
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
        style: ScalarStyle,
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
        style: SequenceStyle,
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
        style: MappingStyle,
    },
    MappingEnd,
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

/// This structure holds information about a potential simple key.
#[derive(Copy, Clone)]
#[non_exhaustive]
pub struct SimpleKey {
    /// Is a simple key possible?
    pub possible: bool,
    /// Is a simple key required?
    pub required: bool,
    /// The number of the token.
    pub token_number: usize,
    /// The position mark.
    pub mark: Mark,
}

/// The states of the parser.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum ParserState {
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
#[non_exhaustive]
pub struct AliasData {
    /// The anchor.
    pub anchor: String,
    /// The node id.
    pub index: i32,
    /// The anchor mark.
    pub mark: Mark,
}

/// The parser structure.
///
/// All members are internal. Manage the structure using the `yaml_parser_`
/// family of functions.
#[non_exhaustive]
pub struct Parser<'r> {
    /// Read handler.
    pub(crate) read_handler: Option<&'r mut dyn std::io::BufRead>,
    /// EOF flag
    pub(crate) eof: bool,
    /// The working buffer.
    ///
    /// This always contains valid UTF-8.
    pub(crate) buffer: VecDeque<char>,
    /// The number of unread characters in the buffer.
    pub(crate) unread: usize,
    /// The input encoding.
    pub(crate) encoding: Encoding,
    /// The offset of the current position (in bytes).
    pub(crate) offset: usize,
    /// The mark of the current position.
    pub(crate) mark: Mark,
    /// Have we started to scan the input stream?
    pub(crate) stream_start_produced: bool,
    /// Have we reached the end of the input stream?
    pub(crate) stream_end_produced: bool,
    /// The number of unclosed '[' and '{' indicators.
    pub(crate) flow_level: i32,
    /// The tokens queue.
    pub(crate) tokens: VecDeque<Token>,
    /// The number of tokens fetched from the queue.
    pub(crate) tokens_parsed: usize,
    /// Does the tokens queue contain a token ready for dequeueing.
    pub(crate) token_available: bool,
    /// The indentation levels stack.
    pub(crate) indents: Vec<i32>,
    /// The current indentation level.
    pub(crate) indent: i32,
    /// May a simple key occur at the current position?
    pub(crate) simple_key_allowed: bool,
    /// The stack of simple keys.
    pub(crate) simple_keys: Vec<SimpleKey>,
    /// The parser states stack.
    pub(crate) states: Vec<ParserState>,
    /// The current parser state.
    pub(crate) state: ParserState,
    /// The stack of marks.
    pub(crate) marks: Vec<Mark>,
    /// The list of TAG directives.
    pub(crate) tag_directives: Vec<TagDirective>,
    /// The alias data.
    pub(crate) aliases: Vec<AliasData>,
}

impl<'r> Default for Parser<'r> {
    fn default() -> Self {
        yaml_parser_new()
    }
}

/// The emitter states.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum EmitterState {
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
pub(crate) struct Anchors {
    /// The number of references.
    pub references: i32,
    /// The anchor id.
    pub anchor: i32,
    /// If the node has been emitted?
    pub serialized: bool,
}

/// The emitter structure.
///
/// All members are internal. Manage the structure using the `yaml_emitter_`
/// family of functions.
#[non_exhaustive]
pub struct Emitter<'w> {
    /// Write handler.
    pub(crate) write_handler: Option<&'w mut dyn std::io::Write>,
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
    pub(crate) encoding: Encoding,
    /// If the output is in the canonical style?
    pub(crate) canonical: bool,
    /// The number of indentation spaces.
    pub(crate) best_indent: i32,
    /// The preferred width of the output lines.
    pub(crate) best_width: i32,
    /// Allow unescaped non-ASCII characters?
    pub(crate) unicode: bool,
    /// The preferred line break.
    pub(crate) line_break: Break,
    /// The stack of states.
    pub(crate) states: Vec<EmitterState>,
    /// The current emitter state.
    pub(crate) state: EmitterState,
    /// The event queue.
    pub(crate) events: VecDeque<Event>,
    /// The stack of indentation levels.
    pub(crate) indents: Vec<i32>,
    /// The list of tag directives.
    pub(crate) tag_directives: Vec<TagDirective>,
    /// The current indentation level.
    pub(crate) indent: i32,
    /// The current flow level.
    pub(crate) flow_level: i32,
    /// Is it the document root context?
    pub(crate) root_context: bool,
    /// Is it a sequence context?
    pub(crate) sequence_context: bool,
    /// Is it a mapping context?
    pub(crate) mapping_context: bool,
    /// Is it a simple mapping key context?
    pub(crate) simple_key_context: bool,
    /// The current line.
    pub(crate) line: i32,
    /// The current column.
    pub(crate) column: i32,
    /// If the last character was a whitespace?
    pub(crate) whitespace: bool,
    /// If the last character was an indentation character (' ', '-', '?', ':')?
    pub(crate) indention: bool,
    /// If an explicit document end is required?
    pub(crate) open_ended: i32,
    /// If the stream was already opened?
    pub(crate) opened: bool,
    /// If the stream was already closed?
    pub(crate) closed: bool,
    /// The information associated with the document nodes.
    // Note: Same length as `document.nodes`.
    pub(crate) anchors: Vec<Anchors>,
    /// The last assigned anchor id.
    pub(crate) last_anchor_id: i32,
}

impl<'a> Default for Emitter<'a> {
    fn default() -> Self {
        yaml_emitter_new()
    }
}
