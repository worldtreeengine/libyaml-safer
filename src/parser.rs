use alloc::string::String;
use alloc::{vec, vec::Vec};

use crate::scanner::Scanner;
use crate::{
    Encoding, Event, EventData, MappingStyle, Mark, ParserError, ScalarStyle, SequenceStyle,
    TagDirective, Token, TokenData, VersionDirective,
};

/// The parser structure.
#[non_exhaustive]
pub struct Parser<'r> {
    pub(crate) scanner: Scanner<'r>,
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
        Self::new()
    }
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
    StreamStart = 0,
    /// Expect the beginning of an implicit document.
    ImplicitDocumentStart = 1,
    /// Expect DOCUMENT-START.
    DocumentStart = 2,
    /// Expect the content of a document.
    DocumentContent = 3,
    /// Expect DOCUMENT-END.
    DocumentEnd = 4,
    /// Expect a block node.
    BlockNode = 5,
    /// Expect a block node or indentless sequence.
    BlockNodeOrIndentlessSequence = 6,
    /// Expect a flow node.
    FlowNode = 7,
    /// Expect the first entry of a block sequence.
    BlockSequenceFirstEntry = 8,
    /// Expect an entry of a block sequence.
    BlockSequenceEntry = 9,
    /// Expect an entry of an indentless sequence.
    IndentlessSequenceEntry = 10,
    /// Expect the first key of a block mapping.
    BlockMappingFirstKey = 11,
    /// Expect a block mapping key.
    BlockMappingKey = 12,
    /// Expect a block mapping value.
    BlockMappingValue = 13,
    /// Expect the first entry of a flow sequence.
    FlowSequenceFirstEntry = 14,
    /// Expect an entry of a flow sequence.
    FlowSequenceEntry = 15,
    /// Expect a key of an ordered mapping.
    FlowSequenceEntryMappingKey = 16,
    /// Expect a value of an ordered mapping.
    FlowSequenceEntryMappingValue = 17,
    /// Expect the and of an ordered mapping entry.
    FlowSequenceEntryMappingEnd = 18,
    /// Expect the first key of a flow mapping.
    FlowMappingFirstKey = 19,
    /// Expect a key of a flow mapping.
    FlowMappingKey = 20,
    /// Expect a value of a flow mapping.
    FlowMappingValue = 21,
    /// Expect an empty value of a flow mapping.
    FlowMappingEmptyValue = 22,
    /// Expect nothing.
    End = 23,
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

fn PEEK_TOKEN<'a>(parser: &'a mut Parser) -> Result<&'a Token, ParserError> {
    if parser.scanner.token_available {
        return Ok(parser
            .scanner
            .tokens
            .front()
            .expect("token_available is true, but token queue is empty"));
    }
    parser.scanner.fetch_more_tokens()?;
    if !parser.scanner.token_available {
        return Err(ParserError::UnexpectedEof);
    }
    Ok(parser
        .scanner
        .tokens
        .front()
        .expect("token_available is true, but token queue is empty"))
}

fn PEEK_TOKEN_MUT<'a>(parser: &'a mut Parser) -> Result<&'a mut Token, ParserError> {
    if parser.scanner.token_available {
        return Ok(parser
            .scanner
            .tokens
            .front_mut()
            .expect("token_available is true, but token queue is empty"));
    }
    parser.scanner.fetch_more_tokens()?;
    if !parser.scanner.token_available {
        return Err(ParserError::UnexpectedEof);
    }
    Ok(parser
        .scanner
        .tokens
        .front_mut()
        .expect("token_available is true, but token queue is empty"))
}

fn SKIP_TOKEN(parser: &mut Parser) {
    parser.scanner.token_available = false;
    parser.scanner.tokens_parsed = parser.scanner.tokens_parsed.wrapping_add(1);
    let skipped = parser
        .scanner
        .tokens
        .pop_front()
        .expect("SKIP_TOKEN but EOF");
    parser.scanner.stream_end_produced = matches!(
        skipped,
        Token {
            data: TokenData::StreamEnd,
            ..
        }
    );
}

impl<'r> Parser<'r> {
    /// Create a parser.
    pub fn new() -> Parser<'r> {
        Parser {
            scanner: Scanner::new(),
            states: Vec::with_capacity(16),
            state: ParserState::default(),
            marks: Vec::with_capacity(16),
            tag_directives: Vec::with_capacity(16),
            aliases: Vec::new(),
        }
    }

    /// Reset the parser state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Set a string input.
    pub fn set_input_string(&mut self, input: &'r mut &[u8]) {
        self.scanner.set_input_string(input);
    }

    /// Set a generic input handler.
    pub fn set_input(&mut self, input: &'r mut dyn std::io::BufRead) {
        self.scanner.set_input(input);
    }

    /// Set the source encoding.
    pub fn set_encoding(&mut self, encoding: Encoding) {
        self.scanner.set_encoding(encoding);
    }

    /// Parse the input stream and produce the next parsing event.
    ///
    /// Call the function subsequently to produce a sequence of events
    /// corresponding to the input stream. The initial event has the type
    /// [`EventData::StreamStart`](crate::EventData::StreamStart) while the
    /// ending event has the type
    /// [`EventData::StreamEnd`](crate::EventData::StreamEnd).
    ///
    /// An application must not alternate the calls of [`Parser::parse()`] with
    /// the calls of [`yaml_parser_scan()`](crate::yaml_parser_scan) or
    /// [`Document::load()`](crate::Document::load). Doing this will break the
    /// parser.
    pub fn parse(&mut self) -> Result<Event, ParserError> {
        if self.scanner.stream_end_produced || self.state == ParserState::End {
            return Ok(Event {
                data: EventData::StreamEnd,
                ..Default::default()
            });
        }
        self.state_machine()
    }

    fn set_parser_error<T>(problem: &'static str, problem_mark: Mark) -> Result<T, ParserError> {
        Err(ParserError::Problem {
            problem,
            mark: problem_mark,
        })
    }

    fn set_parser_error_context<T>(
        context: &'static str,
        context_mark: Mark,
        problem: &'static str,
        problem_mark: Mark,
    ) -> Result<T, ParserError> {
        Err(ParserError::ProblemWithContext {
            context,
            context_mark,
            problem,
            mark: problem_mark,
        })
    }

    fn state_machine(&mut self) -> Result<Event, ParserError> {
        match self.state {
            ParserState::StreamStart => self.parse_stream_start(),
            ParserState::ImplicitDocumentStart => self.parse_document_start(true),
            ParserState::DocumentStart => self.parse_document_start(false),
            ParserState::DocumentContent => self.parse_document_content(),
            ParserState::DocumentEnd => self.parse_document_end(),
            ParserState::BlockNode => self.parse_node(true, false),
            ParserState::BlockNodeOrIndentlessSequence => self.parse_node(true, true),
            ParserState::FlowNode => self.parse_node(false, false),
            ParserState::BlockSequenceFirstEntry => self.parse_block_sequence_entry(true),
            ParserState::BlockSequenceEntry => self.parse_block_sequence_entry(false),
            ParserState::IndentlessSequenceEntry => self.parse_indentless_sequence_entry(),
            ParserState::BlockMappingFirstKey => self.parse_block_mapping_key(true),
            ParserState::BlockMappingKey => self.parse_block_mapping_key(false),
            ParserState::BlockMappingValue => self.parse_block_mapping_value(),
            ParserState::FlowSequenceFirstEntry => self.parse_flow_sequence_entry(true),
            ParserState::FlowSequenceEntry => self.parse_flow_sequence_entry(false),
            ParserState::FlowSequenceEntryMappingKey => {
                self.parse_flow_sequence_entry_mapping_key()
            }
            ParserState::FlowSequenceEntryMappingValue => {
                self.parse_flow_sequence_entry_mapping_value()
            }
            ParserState::FlowSequenceEntryMappingEnd => {
                self.parse_flow_sequence_entry_mapping_end()
            }
            ParserState::FlowMappingFirstKey => self.parse_flow_mapping_key(true),
            ParserState::FlowMappingKey => self.parse_flow_mapping_key(false),
            ParserState::FlowMappingValue => self.parse_flow_mapping_value(false),
            ParserState::FlowMappingEmptyValue => self.parse_flow_mapping_value(true),
            ParserState::End => panic!("parser end state reached unexpectedly"),
        }
    }

    fn parse_stream_start(&mut self) -> Result<Event, ParserError> {
        let token = PEEK_TOKEN(self)?;

        if let TokenData::StreamStart { encoding } = &token.data {
            let event = Event {
                data: EventData::StreamStart {
                    encoding: *encoding,
                },
                start_mark: token.start_mark,
                end_mark: token.end_mark,
            };
            self.state = ParserState::ImplicitDocumentStart;
            SKIP_TOKEN(self);
            Ok(event)
        } else {
            let mark = token.start_mark;
            Self::set_parser_error("did not find expected <stream-start>", mark)
        }
    }

    fn parse_document_start(&mut self, implicit: bool) -> Result<Event, ParserError> {
        let mut version_directive: Option<VersionDirective> = None;

        let mut tag_directives = vec![];
        let mut token = PEEK_TOKEN(self)?;
        if !implicit {
            while let TokenData::DocumentEnd = &token.data {
                SKIP_TOKEN(self);
                token = PEEK_TOKEN(self)?;
            }
        }
        if implicit
            && !matches!(
                token.data,
                TokenData::VersionDirective { .. }
                    | TokenData::TagDirective { .. }
                    | TokenData::DocumentStart
                    | TokenData::StreamEnd
            )
        {
            let event = Event {
                data: EventData::DocumentStart {
                    version_directive: None,
                    tag_directives: vec![],
                    implicit: true,
                },
                start_mark: token.start_mark,
                end_mark: token.end_mark,
            };
            self.process_directives(None, None)?;
            self.states.push(ParserState::DocumentEnd);
            self.state = ParserState::BlockNode;
            Ok(event)
        } else if !matches!(token.data, TokenData::StreamEnd) {
            let end_mark: Mark;
            let start_mark: Mark = token.start_mark;
            self.process_directives(Some(&mut version_directive), Some(&mut tag_directives))?;
            token = PEEK_TOKEN(self)?;
            if let TokenData::DocumentStart = token.data {
                end_mark = token.end_mark;
                let event = Event {
                    data: EventData::DocumentStart {
                        version_directive,
                        tag_directives: core::mem::take(&mut tag_directives),
                        implicit: false,
                    },
                    start_mark,
                    end_mark,
                };
                self.states.push(ParserState::DocumentEnd);
                self.state = ParserState::DocumentContent;
                SKIP_TOKEN(self);
                Ok(event)
            } else {
                Self::set_parser_error("did not find expected <document start>", token.start_mark)
            }
        } else {
            let event = Event {
                data: EventData::StreamEnd,
                start_mark: token.start_mark,
                end_mark: token.end_mark,
            };
            self.state = ParserState::End;
            SKIP_TOKEN(self);
            Ok(event)
        }
    }

    fn parse_document_content(&mut self) -> Result<Event, ParserError> {
        let token = PEEK_TOKEN(self)?;
        if let TokenData::VersionDirective { .. }
        | TokenData::TagDirective { .. }
        | TokenData::DocumentStart
        | TokenData::DocumentEnd
        | TokenData::StreamEnd = &token.data
        {
            let mark = token.start_mark;
            self.state = self.states.pop().unwrap();
            Self::process_empty_scalar(mark)
        } else {
            self.parse_node(true, false)
        }
    }

    fn parse_document_end(&mut self) -> Result<Event, ParserError> {
        let mut end_mark: Mark;
        let mut implicit = true;
        let token = PEEK_TOKEN(self)?;
        end_mark = token.start_mark;
        let start_mark: Mark = end_mark;
        if let TokenData::DocumentEnd = &token.data {
            end_mark = token.end_mark;
            SKIP_TOKEN(self);
            implicit = false;
        }
        self.tag_directives.clear();
        self.state = ParserState::DocumentStart;
        Ok(Event {
            data: EventData::DocumentEnd { implicit },
            start_mark,
            end_mark,
        })
    }

    fn parse_node(&mut self, block: bool, indentless_sequence: bool) -> Result<Event, ParserError> {
        let mut anchor: Option<String> = None;
        let mut tag_handle: Option<String> = None;
        let mut tag_suffix: Option<String> = None;
        let mut tag: Option<String> = None;
        let mut start_mark: Mark;
        let mut end_mark: Mark;
        let mut tag_mark = Mark {
            index: 0,
            line: 0,
            column: 0,
        };

        let mut token = PEEK_TOKEN_MUT(self)?;

        if let TokenData::Alias { value } = &mut token.data {
            let event = Event {
                data: EventData::Alias {
                    anchor: core::mem::take(value),
                },
                start_mark: token.start_mark,
                end_mark: token.end_mark,
            };
            self.state = self.states.pop().unwrap();
            SKIP_TOKEN(self);
            return Ok(event);
        }

        end_mark = token.start_mark;
        start_mark = end_mark;
        if let TokenData::Anchor { value } = &mut token.data {
            anchor = Some(core::mem::take(value));
            start_mark = token.start_mark;
            end_mark = token.end_mark;
            SKIP_TOKEN(self);
            token = PEEK_TOKEN_MUT(self)?;
            if let TokenData::Tag { handle, suffix } = &mut token.data {
                tag_handle = Some(core::mem::take(handle));
                tag_suffix = Some(core::mem::take(suffix));
                tag_mark = token.start_mark;
                end_mark = token.end_mark;
                SKIP_TOKEN(self);
            }
        } else if let TokenData::Tag { handle, suffix } = &mut token.data {
            tag_handle = Some(core::mem::take(handle));
            tag_suffix = Some(core::mem::take(suffix));
            tag_mark = token.start_mark;
            start_mark = tag_mark;
            end_mark = token.end_mark;
            SKIP_TOKEN(self);
            token = PEEK_TOKEN_MUT(self)?;
            if let TokenData::Anchor { value } = &mut token.data {
                anchor = Some(core::mem::take(value));
                end_mark = token.end_mark;
                SKIP_TOKEN(self);
            }
        }

        if let Some(ref tag_handle_value) = tag_handle {
            if tag_handle_value.is_empty() {
                tag = tag_suffix;
            } else {
                for tag_directive in &self.tag_directives {
                    if tag_directive.handle == *tag_handle_value {
                        let suffix = tag_suffix.as_deref().unwrap_or("");
                        tag = Some(alloc::format!("{}{}", tag_directive.prefix, suffix));
                        break;
                    }
                }
                if tag.is_none() {
                    return Self::set_parser_error_context(
                        "while parsing a node",
                        start_mark,
                        "found undefined tag handle",
                        tag_mark,
                    );
                }
            }
        }

        let token = PEEK_TOKEN_MUT(self)?;

        let implicit = tag.is_none() || tag.as_deref() == Some("");

        if indentless_sequence && matches!(token.data, TokenData::BlockEntry) {
            end_mark = token.end_mark;
            self.state = ParserState::IndentlessSequenceEntry;
            let event = Event {
                data: EventData::SequenceStart {
                    anchor,
                    tag,
                    implicit,
                    style: SequenceStyle::Block,
                },
                start_mark,
                end_mark,
            };
            Ok(event)
        } else if let TokenData::Scalar { value, style } = &mut token.data {
            let mut plain_implicit = false;
            let mut quoted_implicit = false;
            end_mark = token.end_mark;
            if *style == ScalarStyle::Plain && tag.is_none() || tag.as_deref() == Some("!") {
                plain_implicit = true;
            } else if tag.is_none() {
                quoted_implicit = true;
            }
            let event = Event {
                data: EventData::Scalar {
                    anchor,
                    tag,
                    value: core::mem::take(value),
                    plain_implicit,
                    quoted_implicit,
                    style: *style,
                },
                start_mark,
                end_mark,
            };
            self.state = self.states.pop().unwrap();
            SKIP_TOKEN(self);
            return Ok(event);
        } else if let TokenData::FlowSequenceStart = &token.data {
            end_mark = token.end_mark;
            self.state = ParserState::FlowSequenceFirstEntry;
            let event = Event {
                data: EventData::SequenceStart {
                    anchor,
                    tag,
                    implicit,
                    style: SequenceStyle::Flow,
                },
                start_mark,
                end_mark,
            };
            return Ok(event);
        } else if let TokenData::FlowMappingStart = &token.data {
            end_mark = token.end_mark;
            self.state = ParserState::FlowMappingFirstKey;
            let event = Event {
                data: EventData::MappingStart {
                    anchor,
                    tag,
                    implicit,
                    style: MappingStyle::Flow,
                },
                start_mark,
                end_mark,
            };
            return Ok(event);
        } else if block && matches!(token.data, TokenData::BlockSequenceStart) {
            end_mark = token.end_mark;
            self.state = ParserState::BlockSequenceFirstEntry;
            let event = Event {
                data: EventData::SequenceStart {
                    anchor,
                    tag,
                    implicit,
                    style: SequenceStyle::Block,
                },
                start_mark,
                end_mark,
            };
            return Ok(event);
        } else if block && matches!(token.data, TokenData::BlockMappingStart) {
            end_mark = token.end_mark;
            self.state = ParserState::BlockMappingFirstKey;
            let event = Event {
                data: EventData::MappingStart {
                    anchor,
                    tag,
                    implicit,
                    style: MappingStyle::Block,
                },
                start_mark,
                end_mark,
            };
            return Ok(event);
        } else if anchor.is_some() || tag.is_some() {
            self.state = self.states.pop().unwrap();
            let event = Event {
                data: EventData::Scalar {
                    anchor,
                    tag,
                    value: String::new(),
                    plain_implicit: implicit,
                    quoted_implicit: false,
                    style: ScalarStyle::Plain,
                },
                start_mark,
                end_mark,
            };
            return Ok(event);
        } else {
            return Self::set_parser_error_context(
                if block {
                    "while parsing a block node"
                } else {
                    "while parsing a flow node"
                },
                start_mark,
                "did not find expected node content",
                token.start_mark,
            );
        }
    }

    fn parse_block_sequence_entry(&mut self, first: bool) -> Result<Event, ParserError> {
        if first {
            let token = PEEK_TOKEN(self)?;
            let mark = token.start_mark;
            self.marks.push(mark);
            SKIP_TOKEN(self);
        }

        let mut token = PEEK_TOKEN(self)?;

        if let TokenData::BlockEntry = &token.data {
            let mark: Mark = token.end_mark;
            SKIP_TOKEN(self);
            token = PEEK_TOKEN(self)?;
            if matches!(token.data, TokenData::BlockEntry | TokenData::BlockEnd) {
                self.state = ParserState::BlockSequenceEntry;
                Self::process_empty_scalar(mark)
            } else {
                self.states.push(ParserState::BlockSequenceEntry);
                self.parse_node(true, false)
            }
        } else if let TokenData::BlockEnd = token.data {
            let event = Event {
                data: EventData::SequenceEnd,
                start_mark: token.start_mark,
                end_mark: token.end_mark,
            };
            self.state = self.states.pop().unwrap();
            let _ = self.marks.pop();
            SKIP_TOKEN(self);
            Ok(event)
        } else {
            let token_mark = token.start_mark;
            let mark = self.marks.pop().unwrap();
            return Self::set_parser_error_context(
                "while parsing a block collection",
                mark,
                "did not find expected '-' indicator",
                token_mark,
            );
        }
    }

    fn parse_indentless_sequence_entry(&mut self) -> Result<Event, ParserError> {
        let mut token = PEEK_TOKEN(self)?;
        if let TokenData::BlockEntry = token.data {
            let mark: Mark = token.end_mark;
            SKIP_TOKEN(self);
            token = PEEK_TOKEN(self)?;

            if matches!(
                token.data,
                TokenData::BlockEntry | TokenData::Key | TokenData::Value | TokenData::BlockEnd
            ) {
                self.state = ParserState::IndentlessSequenceEntry;
                Self::process_empty_scalar(mark)
            } else {
                self.states.push(ParserState::IndentlessSequenceEntry);
                self.parse_node(true, false)
            }
        } else {
            let event = Event {
                data: EventData::SequenceEnd,
                start_mark: token.start_mark,
                end_mark: token.end_mark,
            };
            self.state = self.states.pop().unwrap();
            Ok(event)
        }
    }

    fn parse_block_mapping_key(&mut self, first: bool) -> Result<Event, ParserError> {
        if first {
            let token = PEEK_TOKEN(self)?;
            let mark = token.start_mark;
            self.marks.push(mark);
            SKIP_TOKEN(self);
        }

        let mut token = PEEK_TOKEN(self)?;
        if let TokenData::Key = token.data {
            let mark: Mark = token.end_mark;
            SKIP_TOKEN(self);
            token = PEEK_TOKEN(self)?;
            if matches!(
                token.data,
                TokenData::Key | TokenData::Value | TokenData::BlockEnd
            ) {
                self.state = ParserState::BlockMappingValue;
                Self::process_empty_scalar(mark)
            } else {
                self.states.push(ParserState::BlockMappingValue);
                self.parse_node(true, true)
            }
        } else if let TokenData::BlockEnd = token.data {
            let event = Event {
                data: EventData::MappingEnd,
                start_mark: token.start_mark,
                end_mark: token.end_mark,
            };
            self.state = self.states.pop().unwrap();
            _ = self.marks.pop();
            SKIP_TOKEN(self);
            Ok(event)
        } else {
            let token_mark = token.start_mark;
            let mark = self.marks.pop().unwrap();
            Self::set_parser_error_context(
                "while parsing a block mapping",
                mark,
                "did not find expected key",
                token_mark,
            )
        }
    }

    fn parse_block_mapping_value(&mut self) -> Result<Event, ParserError> {
        let mut token = PEEK_TOKEN(self)?;
        if let TokenData::Value = token.data {
            let mark: Mark = token.end_mark;
            SKIP_TOKEN(self);
            token = PEEK_TOKEN(self)?;
            if matches!(
                token.data,
                TokenData::Key | TokenData::Value | TokenData::BlockEnd
            ) {
                self.state = ParserState::BlockMappingKey;
                Self::process_empty_scalar(mark)
            } else {
                self.states.push(ParserState::BlockMappingKey);
                self.parse_node(true, true)
            }
        } else {
            let mark = token.start_mark;
            self.state = ParserState::BlockMappingKey;
            Self::process_empty_scalar(mark)
        }
    }

    fn parse_flow_sequence_entry(&mut self, first: bool) -> Result<Event, ParserError> {
        if first {
            let token = PEEK_TOKEN(self)?;
            let mark = token.start_mark;
            self.marks.push(mark);
            SKIP_TOKEN(self);
        }

        let mut token = PEEK_TOKEN(self)?;
        if !matches!(token.data, TokenData::FlowSequenceEnd) {
            if !first {
                if let TokenData::FlowEntry = token.data {
                    SKIP_TOKEN(self);
                    token = PEEK_TOKEN(self)?;
                } else {
                    let token_mark = token.start_mark;
                    let mark = self.marks.pop().unwrap();
                    return Self::set_parser_error_context(
                        "while parsing a flow sequence",
                        mark,
                        "did not find expected ',' or ']'",
                        token_mark,
                    );
                }
            }
            if let TokenData::Key = token.data {
                let event = Event {
                    data: EventData::MappingStart {
                        anchor: None,
                        tag: None,
                        implicit: true,
                        style: MappingStyle::Flow,
                    },
                    start_mark: token.start_mark,
                    end_mark: token.end_mark,
                };
                self.state = ParserState::FlowSequenceEntryMappingKey;
                SKIP_TOKEN(self);
                return Ok(event);
            } else if !matches!(token.data, TokenData::FlowSequenceEnd) {
                self.states.push(ParserState::FlowSequenceEntry);
                return self.parse_node(false, false);
            }
        }
        let event = Event {
            data: EventData::SequenceEnd,
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        self.state = self.states.pop().unwrap();
        _ = self.marks.pop();
        SKIP_TOKEN(self);
        Ok(event)
    }

    fn parse_flow_sequence_entry_mapping_key(&mut self) -> Result<Event, ParserError> {
        let token = PEEK_TOKEN(self)?;
        if matches!(
            token.data,
            TokenData::Value | TokenData::FlowEntry | TokenData::FlowSequenceEnd
        ) {
            let mark: Mark = token.end_mark;
            SKIP_TOKEN(self);
            self.state = ParserState::FlowSequenceEntryMappingValue;
            Self::process_empty_scalar(mark)
        } else {
            self.states.push(ParserState::FlowSequenceEntryMappingValue);
            self.parse_node(false, false)
        }
    }

    fn parse_flow_sequence_entry_mapping_value(&mut self) -> Result<Event, ParserError> {
        let mut token = PEEK_TOKEN(self)?;
        if let TokenData::Value = token.data {
            SKIP_TOKEN(self);
            token = PEEK_TOKEN(self)?;
            if !matches!(
                token.data,
                TokenData::FlowEntry | TokenData::FlowSequenceEnd
            ) {
                self.states.push(ParserState::FlowSequenceEntryMappingEnd);
                return self.parse_node(false, false);
            }
        }
        let mark = token.start_mark;
        self.state = ParserState::FlowSequenceEntryMappingEnd;
        Self::process_empty_scalar(mark)
    }

    fn parse_flow_sequence_entry_mapping_end(&mut self) -> Result<Event, ParserError> {
        let token = PEEK_TOKEN(self)?;
        let start_mark = token.start_mark;
        let end_mark = token.end_mark;
        self.state = ParserState::FlowSequenceEntry;
        Ok(Event {
            data: EventData::MappingEnd,
            start_mark,
            end_mark,
        })
    }

    fn parse_flow_mapping_key(&mut self, first: bool) -> Result<Event, ParserError> {
        if first {
            let token = PEEK_TOKEN(self)?;
            let mark = token.start_mark;
            self.marks.push(mark);
            SKIP_TOKEN(self);
        }

        let mut token = PEEK_TOKEN(self)?;
        if !matches!(token.data, TokenData::FlowMappingEnd) {
            if !first {
                if let TokenData::FlowEntry = token.data {
                    SKIP_TOKEN(self);
                    token = PEEK_TOKEN(self)?;
                } else {
                    let token_mark = token.start_mark;
                    let mark = self.marks.pop().unwrap();
                    return Self::set_parser_error_context(
                        "while parsing a flow mapping",
                        mark,
                        "did not find expected ',' or '}'",
                        token_mark,
                    );
                }
            }
            if let TokenData::Key = token.data {
                SKIP_TOKEN(self);
                token = PEEK_TOKEN(self)?;
                if !matches!(
                    token.data,
                    TokenData::Value | TokenData::FlowEntry | TokenData::FlowMappingEnd
                ) {
                    self.states.push(ParserState::FlowMappingValue);
                    return self.parse_node(false, false);
                }
                let mark = token.start_mark;
                self.state = ParserState::FlowMappingValue;
                return Self::process_empty_scalar(mark);
            } else if !matches!(token.data, TokenData::FlowMappingEnd) {
                self.states.push(ParserState::FlowMappingEmptyValue);
                return self.parse_node(false, false);
            }
        }
        let event = Event {
            data: EventData::MappingEnd,
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        self.state = self.states.pop().unwrap();
        _ = self.marks.pop();
        SKIP_TOKEN(self);
        Ok(event)
    }

    fn parse_flow_mapping_value(&mut self, empty: bool) -> Result<Event, ParserError> {
        let mut token = PEEK_TOKEN(self)?;
        if empty {
            let mark = token.start_mark;
            self.state = ParserState::FlowMappingKey;
            return Self::process_empty_scalar(mark);
        }
        if let TokenData::Value = token.data {
            SKIP_TOKEN(self);
            token = PEEK_TOKEN(self)?;
            if !matches!(token.data, TokenData::FlowEntry | TokenData::FlowMappingEnd) {
                self.states.push(ParserState::FlowMappingKey);
                return self.parse_node(false, false);
            }
        }
        let mark = token.start_mark;
        self.state = ParserState::FlowMappingKey;
        Self::process_empty_scalar(mark)
    }

    fn process_empty_scalar(mark: Mark) -> Result<Event, ParserError> {
        Ok(Event {
            data: EventData::Scalar {
                anchor: None,
                tag: None,
                value: String::new(),
                plain_implicit: true,
                quoted_implicit: false,
                style: ScalarStyle::Plain,
            },
            start_mark: mark,
            end_mark: mark,
        })
    }

    fn process_directives(
        &mut self,
        version_directive_ref: Option<&mut Option<VersionDirective>>,
        tag_directives_ref: Option<&mut Vec<TagDirective>>,
    ) -> Result<(), ParserError> {
        let default_tag_directives: [TagDirective; 2] = [
            // TODO: Get rid of these heap allocations.
            TagDirective {
                handle: String::from("!"),
                prefix: String::from("!"),
            },
            TagDirective {
                handle: String::from("!!"),
                prefix: String::from("tag:yaml.org,2002:"),
            },
        ];
        let mut version_directive: Option<VersionDirective> = None;

        let mut tag_directives = Vec::with_capacity(16);

        let mut token = PEEK_TOKEN_MUT(self)?;

        loop {
            if !matches!(
                token.data,
                TokenData::VersionDirective { .. } | TokenData::TagDirective { .. }
            ) {
                break;
            }

            if let TokenData::VersionDirective { major, minor } = &token.data {
                let mark = token.start_mark;
                if version_directive.is_some() {
                    return Self::set_parser_error("found duplicate %YAML directive", mark);
                } else if *major != 1 || *minor != 1 && *minor != 2 {
                    return Self::set_parser_error("found incompatible YAML document", mark);
                }
                version_directive = Some(VersionDirective {
                    major: *major,
                    minor: *minor,
                });
            } else if let TokenData::TagDirective { handle, prefix } = &mut token.data {
                let value = TagDirective {
                    handle: core::mem::take(handle),
                    prefix: core::mem::take(prefix),
                };
                let mark = token.start_mark;
                self.append_tag_directive(value.clone(), false, mark)?;

                tag_directives.push(value);
            }

            SKIP_TOKEN(self);
            token = PEEK_TOKEN_MUT(self)?;
        }

        let start_mark = token.start_mark;
        for default_tag_directive in default_tag_directives {
            self.append_tag_directive(default_tag_directive, true, start_mark)?;
        }

        if let Some(version_directive_ref) = version_directive_ref {
            *version_directive_ref = version_directive;
        }
        if let Some(tag_directives_ref) = tag_directives_ref {
            if tag_directives.is_empty() {
                tag_directives_ref.clear();
                tag_directives.clear();
            } else {
                *tag_directives_ref = tag_directives;
            }
        } else {
            tag_directives.clear();
        }

        Ok(())
    }

    fn append_tag_directive(
        &mut self,
        value: TagDirective,
        allow_duplicates: bool,
        mark: Mark,
    ) -> Result<(), ParserError> {
        for tag_directive in &self.tag_directives {
            if value.handle == tag_directive.handle {
                if allow_duplicates {
                    return Ok(());
                }
                return Self::set_parser_error("found duplicate %TAG directive", mark);
            }
        }
        self.tag_directives.push(value);
        Ok(())
    }

    pub(crate) fn delete_aliases(&mut self) {
        self.aliases.clear();
    }
}
