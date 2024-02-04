use alloc::string::String;
use alloc::{vec, vec::Vec};

use crate::scanner::yaml_parser_fetch_more_tokens;
use crate::yaml::{EventData, TokenData};
use crate::{
    Event, MappingStyle, Mark, Parser, ParserError, ParserState, ScalarStyle, SequenceStyle,
    TagDirective, Token, VersionDirective,
};

fn PEEK_TOKEN<'a>(parser: &'a mut Parser) -> Result<&'a Token, ParserError> {
    if parser.token_available {
        return Ok(parser
            .tokens
            .front()
            .expect("token_available is true, but token queue is empty"));
    }
    yaml_parser_fetch_more_tokens(parser)?;
    if !parser.token_available {
        return Err(ParserError::UnexpectedEof);
    }
    Ok(parser
        .tokens
        .front()
        .expect("token_available is true, but token queue is empty"))
}

fn PEEK_TOKEN_MUT<'a>(parser: &'a mut Parser) -> Result<&'a mut Token, ParserError> {
    if parser.token_available {
        return Ok(parser
            .tokens
            .front_mut()
            .expect("token_available is true, but token queue is empty"));
    }
    yaml_parser_fetch_more_tokens(parser)?;
    if !parser.token_available {
        return Err(ParserError::UnexpectedEof);
    }
    Ok(parser
        .tokens
        .front_mut()
        .expect("token_available is true, but token queue is empty"))
}

fn SKIP_TOKEN(parser: &mut Parser) {
    parser.token_available = false;
    parser.tokens_parsed = parser.tokens_parsed.wrapping_add(1);
    let skipped = parser.tokens.pop_front().expect("SKIP_TOKEN but EOF");
    parser.stream_end_produced = matches!(
        skipped,
        Token {
            data: TokenData::StreamEnd,
            ..
        }
    );
}

/// Parse the input stream and produce the next parsing event.
///
/// Call the function subsequently to produce a sequence of events corresponding
/// to the input stream. The initial event has the type
/// [`EventData::StreamStart`](crate::EventData::StreamStart) while the ending
/// event has the type [`EventData::StreamEnd`](crate::EventData::StreamEnd).
///
/// An application must not alternate the calls of
/// [`yaml_parser_parse()`](crate::yaml_parser_parse) with the calls of
/// [`yaml_parser_scan()`](crate::yaml_parser_scan) or
/// [`yaml_parser_load()`](crate::yaml_parser_load). Doing this will break the
/// parser.
pub fn yaml_parser_parse(parser: &mut Parser) -> Result<Event, ParserError> {
    if parser.stream_end_produced || parser.state == ParserState::End {
        return Ok(Event {
            data: EventData::StreamEnd,
            ..Default::default()
        });
    }
    yaml_parser_state_machine(parser)
}

fn yaml_parser_set_parser_error<T>(
    problem: &'static str,
    problem_mark: Mark,
) -> Result<T, ParserError> {
    Err(ParserError::Problem {
        problem,
        mark: problem_mark,
    })
}

fn yaml_parser_set_parser_error_context<T>(
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

fn yaml_parser_state_machine(parser: &mut Parser) -> Result<Event, ParserError> {
    match parser.state {
        ParserState::StreamStart => yaml_parser_parse_stream_start(parser),
        ParserState::ImplicitDocumentStart => yaml_parser_parse_document_start(parser, true),
        ParserState::DocumentStart => yaml_parser_parse_document_start(parser, false),
        ParserState::DocumentContent => yaml_parser_parse_document_content(parser),
        ParserState::DocumentEnd => yaml_parser_parse_document_end(parser),
        ParserState::BlockNode => yaml_parser_parse_node(parser, true, false),
        ParserState::BlockNodeOrIndentlessSequence => yaml_parser_parse_node(parser, true, true),
        ParserState::FlowNode => yaml_parser_parse_node(parser, false, false),
        ParserState::BlockSequenceFirstEntry => {
            yaml_parser_parse_block_sequence_entry(parser, true)
        }
        ParserState::BlockSequenceEntry => yaml_parser_parse_block_sequence_entry(parser, false),
        ParserState::IndentlessSequenceEntry => yaml_parser_parse_indentless_sequence_entry(parser),
        ParserState::BlockMappingFirstKey => yaml_parser_parse_block_mapping_key(parser, true),
        ParserState::BlockMappingKey => yaml_parser_parse_block_mapping_key(parser, false),
        ParserState::BlockMappingValue => yaml_parser_parse_block_mapping_value(parser),
        ParserState::FlowSequenceFirstEntry => yaml_parser_parse_flow_sequence_entry(parser, true),
        ParserState::FlowSequenceEntry => yaml_parser_parse_flow_sequence_entry(parser, false),
        ParserState::FlowSequenceEntryMappingKey => {
            yaml_parser_parse_flow_sequence_entry_mapping_key(parser)
        }
        ParserState::FlowSequenceEntryMappingValue => {
            yaml_parser_parse_flow_sequence_entry_mapping_value(parser)
        }
        ParserState::FlowSequenceEntryMappingEnd => {
            yaml_parser_parse_flow_sequence_entry_mapping_end(parser)
        }
        ParserState::FlowMappingFirstKey => yaml_parser_parse_flow_mapping_key(parser, true),
        ParserState::FlowMappingKey => yaml_parser_parse_flow_mapping_key(parser, false),
        ParserState::FlowMappingValue => yaml_parser_parse_flow_mapping_value(parser, false),
        ParserState::FlowMappingEmptyValue => yaml_parser_parse_flow_mapping_value(parser, true),
        ParserState::End => panic!("parser end state reached unexpectedly"),
    }
}

fn yaml_parser_parse_stream_start(parser: &mut Parser) -> Result<Event, ParserError> {
    let token = PEEK_TOKEN(parser)?;

    if let TokenData::StreamStart { encoding } = &token.data {
        let event = Event {
            data: EventData::StreamStart {
                encoding: *encoding,
            },
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        parser.state = ParserState::ImplicitDocumentStart;
        SKIP_TOKEN(parser);
        Ok(event)
    } else {
        let mark = token.start_mark;
        yaml_parser_set_parser_error("did not find expected <stream-start>", mark)
    }
}

fn yaml_parser_parse_document_start(
    parser: &mut Parser,
    implicit: bool,
) -> Result<Event, ParserError> {
    let mut version_directive: Option<VersionDirective> = None;

    let mut tag_directives = vec![];
    let mut token = PEEK_TOKEN(parser)?;
    if !implicit {
        while let TokenData::DocumentEnd = &token.data {
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser)?;
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
        yaml_parser_process_directives(parser, None, None)?;
        parser.states.push(ParserState::DocumentEnd);
        parser.state = ParserState::BlockNode;
        Ok(event)
    } else if !matches!(token.data, TokenData::StreamEnd) {
        let end_mark: Mark;
        let start_mark: Mark = token.start_mark;
        yaml_parser_process_directives(
            parser,
            Some(&mut version_directive),
            Some(&mut tag_directives),
        )?;
        token = PEEK_TOKEN(parser)?;
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
            parser.states.push(ParserState::DocumentEnd);
            parser.state = ParserState::DocumentContent;
            SKIP_TOKEN(parser);
            Ok(event)
        } else {
            yaml_parser_set_parser_error("did not find expected <document start>", token.start_mark)
        }
    } else {
        let event = Event {
            data: EventData::StreamEnd,
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        parser.state = ParserState::End;
        SKIP_TOKEN(parser);
        Ok(event)
    }
}

fn yaml_parser_parse_document_content(parser: &mut Parser) -> Result<Event, ParserError> {
    let token = PEEK_TOKEN(parser)?;
    if let TokenData::VersionDirective { .. }
    | TokenData::TagDirective { .. }
    | TokenData::DocumentStart
    | TokenData::DocumentEnd
    | TokenData::StreamEnd = &token.data
    {
        let mark = token.start_mark;
        parser.state = parser.states.pop().unwrap();
        yaml_parser_process_empty_scalar(mark)
    } else {
        yaml_parser_parse_node(parser, true, false)
    }
}

fn yaml_parser_parse_document_end(parser: &mut Parser) -> Result<Event, ParserError> {
    let mut end_mark: Mark;
    let mut implicit = true;
    let token = PEEK_TOKEN(parser)?;
    end_mark = token.start_mark;
    let start_mark: Mark = end_mark;
    if let TokenData::DocumentEnd = &token.data {
        end_mark = token.end_mark;
        SKIP_TOKEN(parser);
        implicit = false;
    }
    parser.tag_directives.clear();
    parser.state = ParserState::DocumentStart;
    Ok(Event {
        data: EventData::DocumentEnd { implicit },
        start_mark,
        end_mark,
    })
}

fn yaml_parser_parse_node(
    parser: &mut Parser,
    block: bool,
    indentless_sequence: bool,
) -> Result<Event, ParserError> {
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

    let mut token = PEEK_TOKEN_MUT(parser)?;

    if let TokenData::Alias { value } = &mut token.data {
        let event = Event {
            data: EventData::Alias {
                anchor: core::mem::take(value),
            },
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        parser.state = parser.states.pop().unwrap();
        SKIP_TOKEN(parser);
        return Ok(event);
    }

    end_mark = token.start_mark;
    start_mark = end_mark;
    if let TokenData::Anchor { value } = &mut token.data {
        anchor = Some(core::mem::take(value));
        start_mark = token.start_mark;
        end_mark = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN_MUT(parser)?;
        if let TokenData::Tag { handle, suffix } = &mut token.data {
            tag_handle = Some(core::mem::take(handle));
            tag_suffix = Some(core::mem::take(suffix));
            tag_mark = token.start_mark;
            end_mark = token.end_mark;
            SKIP_TOKEN(parser);
        }
    } else if let TokenData::Tag { handle, suffix } = &mut token.data {
        tag_handle = Some(core::mem::take(handle));
        tag_suffix = Some(core::mem::take(suffix));
        tag_mark = token.start_mark;
        start_mark = tag_mark;
        end_mark = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN_MUT(parser)?;
        if let TokenData::Anchor { value } = &mut token.data {
            anchor = Some(core::mem::take(value));
            end_mark = token.end_mark;
            SKIP_TOKEN(parser);
        }
    }

    if let Some(ref tag_handle_value) = tag_handle {
        if tag_handle_value.is_empty() {
            tag = tag_suffix;
        } else {
            for tag_directive in &parser.tag_directives {
                if tag_directive.handle == *tag_handle_value {
                    let suffix = tag_suffix.as_deref().unwrap_or("");
                    tag = Some(alloc::format!("{}{}", tag_directive.prefix, suffix));
                    break;
                }
            }
            if tag.is_none() {
                return yaml_parser_set_parser_error_context(
                    "while parsing a node",
                    start_mark,
                    "found undefined tag handle",
                    tag_mark,
                );
            }
        }
    }

    let token = PEEK_TOKEN_MUT(parser)?;

    let implicit = tag.is_none() || tag.as_deref() == Some("");

    if indentless_sequence && matches!(token.data, TokenData::BlockEntry) {
        end_mark = token.end_mark;
        parser.state = ParserState::IndentlessSequenceEntry;
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
        parser.state = parser.states.pop().unwrap();
        SKIP_TOKEN(parser);
        return Ok(event);
    } else if let TokenData::FlowSequenceStart = &token.data {
        end_mark = token.end_mark;
        parser.state = ParserState::FlowSequenceFirstEntry;
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
        parser.state = ParserState::FlowMappingFirstKey;
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
        parser.state = ParserState::BlockSequenceFirstEntry;
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
        parser.state = ParserState::BlockMappingFirstKey;
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
        parser.state = parser.states.pop().unwrap();
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
        return yaml_parser_set_parser_error_context(
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

fn yaml_parser_parse_block_sequence_entry(
    parser: &mut Parser,
    first: bool,
) -> Result<Event, ParserError> {
    if first {
        let token = PEEK_TOKEN(parser)?;
        let mark = token.start_mark;
        parser.marks.push(mark);
        SKIP_TOKEN(parser);
    }

    let mut token = PEEK_TOKEN(parser)?;

    if let TokenData::BlockEntry = &token.data {
        let mark: Mark = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
        if matches!(token.data, TokenData::BlockEntry | TokenData::BlockEnd) {
            parser.state = ParserState::BlockSequenceEntry;
            yaml_parser_process_empty_scalar(mark)
        } else {
            parser.states.push(ParserState::BlockSequenceEntry);
            yaml_parser_parse_node(parser, true, false)
        }
    } else if let TokenData::BlockEnd = token.data {
        let event = Event {
            data: EventData::SequenceEnd,
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        parser.state = parser.states.pop().unwrap();
        let _ = parser.marks.pop();
        SKIP_TOKEN(parser);
        Ok(event)
    } else {
        let token_mark = token.start_mark;
        let mark = parser.marks.pop().unwrap();
        return yaml_parser_set_parser_error_context(
            "while parsing a block collection",
            mark,
            "did not find expected '-' indicator",
            token_mark,
        );
    }
}

fn yaml_parser_parse_indentless_sequence_entry(parser: &mut Parser) -> Result<Event, ParserError> {
    let mut token = PEEK_TOKEN(parser)?;
    if let TokenData::BlockEntry = token.data {
        let mark: Mark = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;

        if matches!(
            token.data,
            TokenData::BlockEntry | TokenData::Key | TokenData::Value | TokenData::BlockEnd
        ) {
            parser.state = ParserState::IndentlessSequenceEntry;
            yaml_parser_process_empty_scalar(mark)
        } else {
            parser.states.push(ParserState::IndentlessSequenceEntry);
            yaml_parser_parse_node(parser, true, false)
        }
    } else {
        let event = Event {
            data: EventData::SequenceEnd,
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        parser.state = parser.states.pop().unwrap();
        Ok(event)
    }
}

fn yaml_parser_parse_block_mapping_key(
    parser: &mut Parser,
    first: bool,
) -> Result<Event, ParserError> {
    if first {
        let token = PEEK_TOKEN(parser)?;
        let mark = token.start_mark;
        parser.marks.push(mark);
        SKIP_TOKEN(parser);
    }

    let mut token = PEEK_TOKEN(parser)?;
    if let TokenData::Key = token.data {
        let mark: Mark = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
        if matches!(
            token.data,
            TokenData::Key | TokenData::Value | TokenData::BlockEnd
        ) {
            parser.state = ParserState::BlockMappingValue;
            yaml_parser_process_empty_scalar(mark)
        } else {
            parser.states.push(ParserState::BlockMappingValue);
            yaml_parser_parse_node(parser, true, true)
        }
    } else if let TokenData::BlockEnd = token.data {
        let event = Event {
            data: EventData::MappingEnd,
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        parser.state = parser.states.pop().unwrap();
        _ = parser.marks.pop();
        SKIP_TOKEN(parser);
        Ok(event)
    } else {
        let token_mark = token.start_mark;
        let mark = parser.marks.pop().unwrap();
        yaml_parser_set_parser_error_context(
            "while parsing a block mapping",
            mark,
            "did not find expected key",
            token_mark,
        )
    }
}

fn yaml_parser_parse_block_mapping_value(parser: &mut Parser) -> Result<Event, ParserError> {
    let mut token = PEEK_TOKEN(parser)?;
    if let TokenData::Value = token.data {
        let mark: Mark = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
        if matches!(
            token.data,
            TokenData::Key | TokenData::Value | TokenData::BlockEnd
        ) {
            parser.state = ParserState::BlockMappingKey;
            yaml_parser_process_empty_scalar(mark)
        } else {
            parser.states.push(ParserState::BlockMappingKey);
            yaml_parser_parse_node(parser, true, true)
        }
    } else {
        let mark = token.start_mark;
        parser.state = ParserState::BlockMappingKey;
        yaml_parser_process_empty_scalar(mark)
    }
}

fn yaml_parser_parse_flow_sequence_entry(
    parser: &mut Parser,
    first: bool,
) -> Result<Event, ParserError> {
    if first {
        let token = PEEK_TOKEN(parser)?;
        let mark = token.start_mark;
        parser.marks.push(mark);
        SKIP_TOKEN(parser);
    }

    let mut token = PEEK_TOKEN(parser)?;
    if !matches!(token.data, TokenData::FlowSequenceEnd) {
        if !first {
            if let TokenData::FlowEntry = token.data {
                SKIP_TOKEN(parser);
                token = PEEK_TOKEN(parser)?;
            } else {
                let token_mark = token.start_mark;
                let mark = parser.marks.pop().unwrap();
                return yaml_parser_set_parser_error_context(
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
            parser.state = ParserState::FlowSequenceEntryMappingKey;
            SKIP_TOKEN(parser);
            return Ok(event);
        } else if !matches!(token.data, TokenData::FlowSequenceEnd) {
            parser.states.push(ParserState::FlowSequenceEntry);
            return yaml_parser_parse_node(parser, false, false);
        }
    }
    let event = Event {
        data: EventData::SequenceEnd,
        start_mark: token.start_mark,
        end_mark: token.end_mark,
    };
    parser.state = parser.states.pop().unwrap();
    _ = parser.marks.pop();
    SKIP_TOKEN(parser);
    Ok(event)
}

fn yaml_parser_parse_flow_sequence_entry_mapping_key(
    parser: &mut Parser,
) -> Result<Event, ParserError> {
    let token = PEEK_TOKEN(parser)?;
    if matches!(
        token.data,
        TokenData::Value | TokenData::FlowEntry | TokenData::FlowSequenceEnd
    ) {
        let mark: Mark = token.end_mark;
        SKIP_TOKEN(parser);
        parser.state = ParserState::FlowSequenceEntryMappingValue;
        yaml_parser_process_empty_scalar(mark)
    } else {
        parser
            .states
            .push(ParserState::FlowSequenceEntryMappingValue);
        yaml_parser_parse_node(parser, false, false)
    }
}

fn yaml_parser_parse_flow_sequence_entry_mapping_value(
    parser: &mut Parser,
) -> Result<Event, ParserError> {
    let mut token = PEEK_TOKEN(parser)?;
    if let TokenData::Value = token.data {
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
        if !matches!(
            token.data,
            TokenData::FlowEntry | TokenData::FlowSequenceEnd
        ) {
            parser.states.push(ParserState::FlowSequenceEntryMappingEnd);
            return yaml_parser_parse_node(parser, false, false);
        }
    }
    let mark = token.start_mark;
    parser.state = ParserState::FlowSequenceEntryMappingEnd;
    yaml_parser_process_empty_scalar(mark)
}

fn yaml_parser_parse_flow_sequence_entry_mapping_end(
    parser: &mut Parser,
) -> Result<Event, ParserError> {
    let token = PEEK_TOKEN(parser)?;
    let start_mark = token.start_mark;
    let end_mark = token.end_mark;
    parser.state = ParserState::FlowSequenceEntry;
    Ok(Event {
        data: EventData::MappingEnd,
        start_mark,
        end_mark,
    })
}

fn yaml_parser_parse_flow_mapping_key(
    parser: &mut Parser,
    first: bool,
) -> Result<Event, ParserError> {
    if first {
        let token = PEEK_TOKEN(parser)?;
        let mark = token.start_mark;
        parser.marks.push(mark);
        SKIP_TOKEN(parser);
    }

    let mut token = PEEK_TOKEN(parser)?;
    if !matches!(token.data, TokenData::FlowMappingEnd) {
        if !first {
            if let TokenData::FlowEntry = token.data {
                SKIP_TOKEN(parser);
                token = PEEK_TOKEN(parser)?;
            } else {
                let token_mark = token.start_mark;
                let mark = parser.marks.pop().unwrap();
                return yaml_parser_set_parser_error_context(
                    "while parsing a flow mapping",
                    mark,
                    "did not find expected ',' or '}'",
                    token_mark,
                );
            }
        }
        if let TokenData::Key = token.data {
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser)?;
            if !matches!(
                token.data,
                TokenData::Value | TokenData::FlowEntry | TokenData::FlowMappingEnd
            ) {
                parser.states.push(ParserState::FlowMappingValue);
                return yaml_parser_parse_node(parser, false, false);
            }
            let mark = token.start_mark;
            parser.state = ParserState::FlowMappingValue;
            return yaml_parser_process_empty_scalar(mark);
        } else if !matches!(token.data, TokenData::FlowMappingEnd) {
            parser.states.push(ParserState::FlowMappingEmptyValue);
            return yaml_parser_parse_node(parser, false, false);
        }
    }
    let event = Event {
        data: EventData::MappingEnd,
        start_mark: token.start_mark,
        end_mark: token.end_mark,
    };
    parser.state = parser.states.pop().unwrap();
    _ = parser.marks.pop();
    SKIP_TOKEN(parser);
    Ok(event)
}

fn yaml_parser_parse_flow_mapping_value(
    parser: &mut Parser,
    empty: bool,
) -> Result<Event, ParserError> {
    let mut token = PEEK_TOKEN(parser)?;
    if empty {
        let mark = token.start_mark;
        parser.state = ParserState::FlowMappingKey;
        return yaml_parser_process_empty_scalar(mark);
    }
    if let TokenData::Value = token.data {
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
        if !matches!(token.data, TokenData::FlowEntry | TokenData::FlowMappingEnd) {
            parser.states.push(ParserState::FlowMappingKey);
            return yaml_parser_parse_node(parser, false, false);
        }
    }
    let mark = token.start_mark;
    parser.state = ParserState::FlowMappingKey;
    yaml_parser_process_empty_scalar(mark)
}

fn yaml_parser_process_empty_scalar(mark: Mark) -> Result<Event, ParserError> {
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

fn yaml_parser_process_directives(
    parser: &mut Parser,
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

    let mut token = PEEK_TOKEN(parser)?;

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
                return yaml_parser_set_parser_error("found duplicate %YAML directive", mark);
            } else if *major != 1 || *minor != 1 && *minor != 2 {
                return yaml_parser_set_parser_error("found incompatible YAML document", mark);
            }
            version_directive = Some(VersionDirective {
                major: *major,
                minor: *minor,
            });
        } else if let TokenData::TagDirective { handle, prefix } = &token.data {
            let value = TagDirective {
                // TODO: Get rid of these clones by consuming tokens by value.
                handle: handle.clone(),
                prefix: prefix.clone(),
            };
            let mark = token.start_mark;
            yaml_parser_append_tag_directive(parser, &value, false, mark)?;

            tag_directives.push(value);
        }

        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
    }

    let start_mark = token.start_mark;
    for default_tag_directive in &default_tag_directives {
        yaml_parser_append_tag_directive(parser, default_tag_directive, true, start_mark)?;
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

fn yaml_parser_append_tag_directive(
    parser: &mut Parser,
    value: &TagDirective,
    allow_duplicates: bool,
    mark: Mark,
) -> Result<(), ParserError> {
    for tag_directive in &parser.tag_directives {
        if value.handle == tag_directive.handle {
            if allow_duplicates {
                return Ok(());
            }
            return yaml_parser_set_parser_error("found duplicate %TAG directive", mark);
        }
    }
    parser.tag_directives.push(value.clone());
    Ok(())
}
