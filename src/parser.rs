use alloc::string::String;
use alloc::{vec, vec::Vec};

use crate::scanner::yaml_parser_fetch_more_tokens;
use crate::yaml::{YamlEventData, YamlTokenData};
use crate::{
    yaml_event_t, yaml_mark_t, yaml_parser_t, yaml_tag_directive_t, yaml_token_t,
    yaml_version_directive_t, ParserError, YAML_BLOCK_MAPPING_STYLE, YAML_BLOCK_SEQUENCE_STYLE,
    YAML_FLOW_MAPPING_STYLE, YAML_FLOW_SEQUENCE_STYLE, YAML_PARSE_BLOCK_MAPPING_FIRST_KEY_STATE,
    YAML_PARSE_BLOCK_MAPPING_KEY_STATE, YAML_PARSE_BLOCK_MAPPING_VALUE_STATE,
    YAML_PARSE_BLOCK_NODE_OR_INDENTLESS_SEQUENCE_STATE, YAML_PARSE_BLOCK_NODE_STATE,
    YAML_PARSE_BLOCK_SEQUENCE_ENTRY_STATE, YAML_PARSE_BLOCK_SEQUENCE_FIRST_ENTRY_STATE,
    YAML_PARSE_DOCUMENT_CONTENT_STATE, YAML_PARSE_DOCUMENT_END_STATE,
    YAML_PARSE_DOCUMENT_START_STATE, YAML_PARSE_END_STATE,
    YAML_PARSE_FLOW_MAPPING_EMPTY_VALUE_STATE, YAML_PARSE_FLOW_MAPPING_FIRST_KEY_STATE,
    YAML_PARSE_FLOW_MAPPING_KEY_STATE, YAML_PARSE_FLOW_MAPPING_VALUE_STATE,
    YAML_PARSE_FLOW_NODE_STATE, YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_END_STATE,
    YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_KEY_STATE,
    YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_VALUE_STATE, YAML_PARSE_FLOW_SEQUENCE_ENTRY_STATE,
    YAML_PARSE_FLOW_SEQUENCE_FIRST_ENTRY_STATE, YAML_PARSE_IMPLICIT_DOCUMENT_START_STATE,
    YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE, YAML_PARSE_STREAM_START_STATE,
    YAML_PLAIN_SCALAR_STYLE,
};

fn PEEK_TOKEN<'a>(parser: &'a mut yaml_parser_t) -> Result<&'a yaml_token_t, ParserError> {
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

fn PEEK_TOKEN_MUT<'a>(parser: &'a mut yaml_parser_t) -> Result<&'a mut yaml_token_t, ParserError> {
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

fn SKIP_TOKEN(parser: &mut yaml_parser_t) {
    parser.token_available = false;
    parser.tokens_parsed = parser.tokens_parsed.wrapping_add(1);
    let skipped = parser.tokens.pop_front().expect("SKIP_TOKEN but EOF");
    parser.stream_end_produced = matches!(
        skipped,
        yaml_token_t {
            data: YamlTokenData::StreamEnd,
            ..
        }
    );
}

/// Parse the input stream and produce the next parsing event.
///
/// Call the function subsequently to produce a sequence of events corresponding
/// to the input stream. The initial event has the type YAML_STREAM_START_EVENT
/// while the ending event has the type YAML_STREAM_END_EVENT.
///
/// An application must not alternate the calls of yaml_parser_parse() with the
/// calls of yaml_parser_scan() or yaml_parser_load(). Doing this will break the
/// parser.
pub fn yaml_parser_parse(parser: &mut yaml_parser_t) -> Result<yaml_event_t, ParserError> {
    if parser.stream_end_produced || parser.state == YAML_PARSE_END_STATE {
        return Ok(yaml_event_t {
            data: YamlEventData::StreamEnd,
            ..Default::default()
        });
    }
    yaml_parser_state_machine(parser)
}

fn yaml_parser_set_parser_error<T>(
    problem: &'static str,
    problem_mark: yaml_mark_t,
) -> Result<T, ParserError> {
    Err(ParserError::Problem {
        problem,
        mark: problem_mark,
    })
}

fn yaml_parser_set_parser_error_context<T>(
    context: &'static str,
    context_mark: yaml_mark_t,
    problem: &'static str,
    problem_mark: yaml_mark_t,
) -> Result<T, ParserError> {
    Err(ParserError::ProblemWithContext {
        context,
        context_mark,
        problem,
        mark: problem_mark,
    })
}

fn yaml_parser_state_machine(parser: &mut yaml_parser_t) -> Result<yaml_event_t, ParserError> {
    match parser.state {
        YAML_PARSE_STREAM_START_STATE => yaml_parser_parse_stream_start(parser),
        YAML_PARSE_IMPLICIT_DOCUMENT_START_STATE => yaml_parser_parse_document_start(parser, true),
        YAML_PARSE_DOCUMENT_START_STATE => yaml_parser_parse_document_start(parser, false),
        YAML_PARSE_DOCUMENT_CONTENT_STATE => yaml_parser_parse_document_content(parser),
        YAML_PARSE_DOCUMENT_END_STATE => yaml_parser_parse_document_end(parser),
        YAML_PARSE_BLOCK_NODE_STATE => yaml_parser_parse_node(parser, true, false),
        YAML_PARSE_BLOCK_NODE_OR_INDENTLESS_SEQUENCE_STATE => {
            yaml_parser_parse_node(parser, true, true)
        }
        YAML_PARSE_FLOW_NODE_STATE => yaml_parser_parse_node(parser, false, false),
        YAML_PARSE_BLOCK_SEQUENCE_FIRST_ENTRY_STATE => {
            yaml_parser_parse_block_sequence_entry(parser, true)
        }
        YAML_PARSE_BLOCK_SEQUENCE_ENTRY_STATE => {
            yaml_parser_parse_block_sequence_entry(parser, false)
        }
        YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE => {
            yaml_parser_parse_indentless_sequence_entry(parser)
        }
        YAML_PARSE_BLOCK_MAPPING_FIRST_KEY_STATE => {
            yaml_parser_parse_block_mapping_key(parser, true)
        }
        YAML_PARSE_BLOCK_MAPPING_KEY_STATE => yaml_parser_parse_block_mapping_key(parser, false),
        YAML_PARSE_BLOCK_MAPPING_VALUE_STATE => yaml_parser_parse_block_mapping_value(parser),
        YAML_PARSE_FLOW_SEQUENCE_FIRST_ENTRY_STATE => {
            yaml_parser_parse_flow_sequence_entry(parser, true)
        }
        YAML_PARSE_FLOW_SEQUENCE_ENTRY_STATE => {
            yaml_parser_parse_flow_sequence_entry(parser, false)
        }
        YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_KEY_STATE => {
            yaml_parser_parse_flow_sequence_entry_mapping_key(parser)
        }
        YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_VALUE_STATE => {
            yaml_parser_parse_flow_sequence_entry_mapping_value(parser)
        }
        YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_END_STATE => {
            yaml_parser_parse_flow_sequence_entry_mapping_end(parser)
        }
        YAML_PARSE_FLOW_MAPPING_FIRST_KEY_STATE => yaml_parser_parse_flow_mapping_key(parser, true),
        YAML_PARSE_FLOW_MAPPING_KEY_STATE => yaml_parser_parse_flow_mapping_key(parser, false),
        YAML_PARSE_FLOW_MAPPING_VALUE_STATE => yaml_parser_parse_flow_mapping_value(parser, false),
        YAML_PARSE_FLOW_MAPPING_EMPTY_VALUE_STATE => {
            yaml_parser_parse_flow_mapping_value(parser, true)
        }
        YAML_PARSE_END_STATE => panic!("parser end state reached unexpectedly"),
    }
}

fn yaml_parser_parse_stream_start(parser: &mut yaml_parser_t) -> Result<yaml_event_t, ParserError> {
    let token = PEEK_TOKEN(parser)?;

    if let YamlTokenData::StreamStart { encoding } = &token.data {
        let event = yaml_event_t {
            data: YamlEventData::StreamStart {
                encoding: *encoding,
            },
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        parser.state = YAML_PARSE_IMPLICIT_DOCUMENT_START_STATE;
        SKIP_TOKEN(parser);
        Ok(event)
    } else {
        let mark = token.start_mark;
        yaml_parser_set_parser_error("did not find expected <stream-start>", mark)
    }
}

fn yaml_parser_parse_document_start(
    parser: &mut yaml_parser_t,
    implicit: bool,
) -> Result<yaml_event_t, ParserError> {
    let mut version_directive: Option<yaml_version_directive_t> = None;

    let mut tag_directives = vec![];
    let mut token = PEEK_TOKEN(parser)?;
    if !implicit {
        while let YamlTokenData::DocumentEnd = &token.data {
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser)?;
        }
    }
    if implicit
        && !token.data.is_version_directive()
        && !token.data.is_tag_directive()
        && !token.data.is_document_start()
        && !token.data.is_stream_end()
    {
        let event = yaml_event_t {
            data: YamlEventData::DocumentStart {
                version_directive: None,
                tag_directives: vec![],
                implicit: true,
            },
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        yaml_parser_process_directives(parser, None, None)?;
        parser.states.push(YAML_PARSE_DOCUMENT_END_STATE);
        parser.state = YAML_PARSE_BLOCK_NODE_STATE;
        Ok(event)
    } else if !token.data.is_stream_end() {
        let end_mark: yaml_mark_t;
        let start_mark: yaml_mark_t = token.start_mark;
        yaml_parser_process_directives(
            parser,
            Some(&mut version_directive),
            Some(&mut tag_directives),
        )?;
        token = PEEK_TOKEN(parser)?;
        if !token.data.is_document_start() {
            return yaml_parser_set_parser_error(
                "did not find expected <document start>",
                token.start_mark,
            );
        } else {
            end_mark = token.end_mark;
            let event = yaml_event_t {
                data: YamlEventData::DocumentStart {
                    version_directive,
                    tag_directives: core::mem::take(&mut tag_directives),
                    implicit: false,
                },
                start_mark,
                end_mark,
            };
            parser.states.push(YAML_PARSE_DOCUMENT_END_STATE);
            parser.state = YAML_PARSE_DOCUMENT_CONTENT_STATE;
            SKIP_TOKEN(parser);
            return Ok(event);
        }
    } else {
        let event = yaml_event_t {
            data: YamlEventData::StreamEnd,
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        parser.state = YAML_PARSE_END_STATE;
        SKIP_TOKEN(parser);
        Ok(event)
    }
}

fn yaml_parser_parse_document_content(
    parser: &mut yaml_parser_t,
) -> Result<yaml_event_t, ParserError> {
    let token = PEEK_TOKEN(parser)?;
    if let YamlTokenData::VersionDirective { .. }
    | YamlTokenData::TagDirective { .. }
    | YamlTokenData::DocumentStart
    | YamlTokenData::DocumentEnd
    | YamlTokenData::StreamEnd = &token.data
    {
        let mark = token.start_mark;
        parser.state = parser.states.pop().unwrap();
        yaml_parser_process_empty_scalar(mark)
    } else {
        yaml_parser_parse_node(parser, true, false)
    }
}

fn yaml_parser_parse_document_end(parser: &mut yaml_parser_t) -> Result<yaml_event_t, ParserError> {
    let mut end_mark: yaml_mark_t;
    let mut implicit = true;
    let token = PEEK_TOKEN(parser)?;
    end_mark = token.start_mark;
    let start_mark: yaml_mark_t = end_mark;
    if let YamlTokenData::DocumentEnd = &token.data {
        end_mark = token.end_mark;
        SKIP_TOKEN(parser);
        implicit = false;
    }
    parser.tag_directives.clear();
    parser.state = YAML_PARSE_DOCUMENT_START_STATE;
    Ok(yaml_event_t {
        data: YamlEventData::DocumentEnd { implicit },
        start_mark,
        end_mark,
    })
}

fn yaml_parser_parse_node(
    parser: &mut yaml_parser_t,
    block: bool,
    indentless_sequence: bool,
) -> Result<yaml_event_t, ParserError> {
    let mut anchor: Option<String> = None;
    let mut tag_handle: Option<String> = None;
    let mut tag_suffix: Option<String> = None;
    let mut tag: Option<String> = None;
    let mut start_mark: yaml_mark_t;
    let mut end_mark: yaml_mark_t;
    let mut tag_mark = yaml_mark_t {
        index: 0,
        line: 0,
        column: 0,
    };

    let mut token = PEEK_TOKEN_MUT(parser)?;

    if let YamlTokenData::Alias { value } = &mut token.data {
        let event = yaml_event_t {
            data: YamlEventData::Alias {
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
    if let YamlTokenData::Anchor { value } = &mut token.data {
        anchor = Some(core::mem::take(value));
        start_mark = token.start_mark;
        end_mark = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN_MUT(parser)?;
        if let YamlTokenData::Tag { handle, suffix } = &mut token.data {
            tag_handle = Some(core::mem::take(handle));
            tag_suffix = Some(core::mem::take(suffix));
            tag_mark = token.start_mark;
            end_mark = token.end_mark;
            SKIP_TOKEN(parser);
        }
    } else if let YamlTokenData::Tag { handle, suffix } = &mut token.data {
        tag_handle = Some(core::mem::take(handle));
        tag_suffix = Some(core::mem::take(suffix));
        tag_mark = token.start_mark;
        start_mark = tag_mark;
        end_mark = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN_MUT(parser)?;
        if let YamlTokenData::Anchor { value } = &mut token.data {
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

    if indentless_sequence && token.data.is_block_entry() {
        end_mark = token.end_mark;
        parser.state = YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE;
        let event = yaml_event_t {
            data: YamlEventData::SequenceStart {
                anchor,
                tag,
                implicit,
                style: YAML_BLOCK_SEQUENCE_STYLE,
            },
            start_mark,
            end_mark,
        };
        Ok(event)
    } else if let YamlTokenData::Scalar { value, style } = &mut token.data {
        let mut plain_implicit = false;
        let mut quoted_implicit = false;
        end_mark = token.end_mark;
        if *style == YAML_PLAIN_SCALAR_STYLE && tag.is_none() || tag.as_deref() == Some("!") {
            plain_implicit = true;
        } else if tag.is_none() {
            quoted_implicit = true;
        }
        let event = yaml_event_t {
            data: YamlEventData::Scalar {
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
    } else if let YamlTokenData::FlowSequenceStart = &token.data {
        end_mark = token.end_mark;
        parser.state = YAML_PARSE_FLOW_SEQUENCE_FIRST_ENTRY_STATE;
        let event = yaml_event_t {
            data: YamlEventData::SequenceStart {
                anchor,
                tag,
                implicit,
                style: YAML_FLOW_SEQUENCE_STYLE,
            },
            start_mark,
            end_mark,
        };
        return Ok(event);
    } else if let YamlTokenData::FlowMappingStart = &token.data {
        end_mark = token.end_mark;
        parser.state = YAML_PARSE_FLOW_MAPPING_FIRST_KEY_STATE;
        let event = yaml_event_t {
            data: YamlEventData::MappingStart {
                anchor,
                tag,
                implicit,
                style: YAML_FLOW_MAPPING_STYLE,
            },
            start_mark,
            end_mark,
        };
        return Ok(event);
    } else if block && token.data.is_block_sequence_start() {
        end_mark = token.end_mark;
        parser.state = YAML_PARSE_BLOCK_SEQUENCE_FIRST_ENTRY_STATE;
        let event = yaml_event_t {
            data: YamlEventData::SequenceStart {
                anchor,
                tag,
                implicit,
                style: YAML_BLOCK_SEQUENCE_STYLE,
            },
            start_mark,
            end_mark,
        };
        return Ok(event);
    } else if block && token.data.is_block_mapping_start() {
        end_mark = token.end_mark;
        parser.state = YAML_PARSE_BLOCK_MAPPING_FIRST_KEY_STATE;
        let event = yaml_event_t {
            data: YamlEventData::MappingStart {
                anchor,
                tag,
                implicit,
                style: YAML_BLOCK_MAPPING_STYLE,
            },
            start_mark,
            end_mark,
        };
        return Ok(event);
    } else if anchor.is_some() || tag.is_some() {
        parser.state = parser.states.pop().unwrap();
        let event = yaml_event_t {
            data: YamlEventData::Scalar {
                anchor,
                tag,
                value: String::new(),
                plain_implicit: implicit,
                quoted_implicit: false,
                style: YAML_PLAIN_SCALAR_STYLE,
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
    parser: &mut yaml_parser_t,
    first: bool,
) -> Result<yaml_event_t, ParserError> {
    if first {
        let token = PEEK_TOKEN(parser)?;
        let mark = token.start_mark;
        parser.marks.push(mark);
        SKIP_TOKEN(parser);
    }

    let mut token = PEEK_TOKEN(parser)?;

    if let YamlTokenData::BlockEntry = &token.data {
        let mark: yaml_mark_t = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
        if !token.data.is_block_entry() && !token.data.is_block_end() {
            parser.states.push(YAML_PARSE_BLOCK_SEQUENCE_ENTRY_STATE);
            yaml_parser_parse_node(parser, true, false)
        } else {
            parser.state = YAML_PARSE_BLOCK_SEQUENCE_ENTRY_STATE;
            yaml_parser_process_empty_scalar(mark)
        }
    } else if token.data.is_block_end() {
        let event = yaml_event_t {
            data: YamlEventData::SequenceEnd,
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

fn yaml_parser_parse_indentless_sequence_entry(
    parser: &mut yaml_parser_t,
) -> Result<yaml_event_t, ParserError> {
    let mut token = PEEK_TOKEN(parser)?;
    if token.data.is_block_entry() {
        let mark: yaml_mark_t = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
        if !token.data.is_block_entry()
            && !token.data.is_key()
            && !token.data.is_value()
            && !token.data.is_block_end()
        {
            parser
                .states
                .push(YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE);
            yaml_parser_parse_node(parser, true, false)
        } else {
            parser.state = YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE;
            yaml_parser_process_empty_scalar(mark)
        }
    } else {
        let event = yaml_event_t {
            data: YamlEventData::SequenceEnd,
            start_mark: token.start_mark,
            end_mark: token.end_mark,
        };
        parser.state = parser.states.pop().unwrap();
        Ok(event)
    }
}

fn yaml_parser_parse_block_mapping_key(
    parser: &mut yaml_parser_t,
    first: bool,
) -> Result<yaml_event_t, ParserError> {
    if first {
        let token = PEEK_TOKEN(parser)?;
        let mark = token.start_mark;
        parser.marks.push(mark);
        SKIP_TOKEN(parser);
    }

    let mut token = PEEK_TOKEN(parser)?;
    if token.data.is_key() {
        let mark: yaml_mark_t = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
        if !token.data.is_key() && !token.data.is_value() && !token.data.is_block_end() {
            parser.states.push(YAML_PARSE_BLOCK_MAPPING_VALUE_STATE);
            yaml_parser_parse_node(parser, true, true)
        } else {
            parser.state = YAML_PARSE_BLOCK_MAPPING_VALUE_STATE;
            yaml_parser_process_empty_scalar(mark)
        }
    } else if token.data.is_block_end() {
        let event = yaml_event_t {
            data: YamlEventData::MappingEnd,
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

fn yaml_parser_parse_block_mapping_value(
    parser: &mut yaml_parser_t,
) -> Result<yaml_event_t, ParserError> {
    let mut token = PEEK_TOKEN(parser)?;
    if token.data.is_value() {
        let mark: yaml_mark_t = token.end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
        if !token.data.is_key() && !token.data.is_value() && !token.data.is_block_end() {
            parser.states.push(YAML_PARSE_BLOCK_MAPPING_KEY_STATE);
            yaml_parser_parse_node(parser, true, true)
        } else {
            parser.state = YAML_PARSE_BLOCK_MAPPING_KEY_STATE;
            yaml_parser_process_empty_scalar(mark)
        }
    } else {
        let mark = token.start_mark;
        parser.state = YAML_PARSE_BLOCK_MAPPING_KEY_STATE;
        yaml_parser_process_empty_scalar(mark)
    }
}

fn yaml_parser_parse_flow_sequence_entry(
    parser: &mut yaml_parser_t,
    first: bool,
) -> Result<yaml_event_t, ParserError> {
    if first {
        let token = PEEK_TOKEN(parser)?;
        let mark = token.start_mark;
        parser.marks.push(mark);
        SKIP_TOKEN(parser);
    }

    let mut token = PEEK_TOKEN(parser)?;
    if !token.data.is_flow_sequence_end() {
        if !first {
            if token.data.is_flow_entry() {
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
        if token.data.is_key() {
            let event = yaml_event_t {
                data: YamlEventData::MappingStart {
                    anchor: None,
                    tag: None,
                    implicit: true,
                    style: YAML_FLOW_MAPPING_STYLE,
                },
                start_mark: token.start_mark,
                end_mark: token.end_mark,
            };
            parser.state = YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_KEY_STATE;
            SKIP_TOKEN(parser);
            return Ok(event);
        } else if !token.data.is_flow_sequence_end() {
            parser.states.push(YAML_PARSE_FLOW_SEQUENCE_ENTRY_STATE);
            return yaml_parser_parse_node(parser, false, false);
        }
    }
    let event = yaml_event_t {
        data: YamlEventData::SequenceEnd,
        start_mark: token.start_mark,
        end_mark: token.end_mark,
    };
    parser.state = parser.states.pop().unwrap();
    _ = parser.marks.pop();
    SKIP_TOKEN(parser);
    Ok(event)
}

fn yaml_parser_parse_flow_sequence_entry_mapping_key(
    parser: &mut yaml_parser_t,
) -> Result<yaml_event_t, ParserError> {
    let token = PEEK_TOKEN(parser)?;
    if !token.data.is_value() && !token.data.is_flow_entry() && !token.data.is_flow_sequence_end() {
        parser
            .states
            .push(YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_VALUE_STATE);
        yaml_parser_parse_node(parser, false, false)
    } else {
        let mark: yaml_mark_t = token.end_mark;
        SKIP_TOKEN(parser);
        parser.state = YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_VALUE_STATE;
        yaml_parser_process_empty_scalar(mark)
    }
}

fn yaml_parser_parse_flow_sequence_entry_mapping_value(
    parser: &mut yaml_parser_t,
) -> Result<yaml_event_t, ParserError> {
    let mut token = PEEK_TOKEN(parser)?;
    if token.data.is_value() {
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
        if !token.data.is_flow_entry() && !token.data.is_flow_sequence_end() {
            parser
                .states
                .push(YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_END_STATE);
            return yaml_parser_parse_node(parser, false, false);
        }
    }
    let mark = token.start_mark;
    parser.state = YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_END_STATE;
    yaml_parser_process_empty_scalar(mark)
}

fn yaml_parser_parse_flow_sequence_entry_mapping_end(
    parser: &mut yaml_parser_t,
) -> Result<yaml_event_t, ParserError> {
    let token = PEEK_TOKEN(parser)?;
    let start_mark = token.start_mark;
    let end_mark = token.end_mark;
    parser.state = YAML_PARSE_FLOW_SEQUENCE_ENTRY_STATE;
    Ok(yaml_event_t {
        data: YamlEventData::MappingEnd,
        start_mark,
        end_mark,
    })
}

fn yaml_parser_parse_flow_mapping_key(
    parser: &mut yaml_parser_t,
    first: bool,
) -> Result<yaml_event_t, ParserError> {
    if first {
        let token = PEEK_TOKEN(parser)?;
        let mark = token.start_mark;
        parser.marks.push(mark);
        SKIP_TOKEN(parser);
    }

    let mut token = PEEK_TOKEN(parser)?;
    if !token.data.is_flow_mapping_end() {
        if !first {
            if token.data.is_flow_entry() {
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
        if token.data.is_key() {
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser)?;
            if !token.data.is_value()
                && !token.data.is_flow_entry()
                && !token.data.is_flow_mapping_end()
            {
                parser.states.push(YAML_PARSE_FLOW_MAPPING_VALUE_STATE);
                return yaml_parser_parse_node(parser, false, false);
            } else {
                let mark = token.start_mark;
                parser.state = YAML_PARSE_FLOW_MAPPING_VALUE_STATE;
                return yaml_parser_process_empty_scalar(mark);
            }
        } else if !token.data.is_flow_mapping_end() {
            parser
                .states
                .push(YAML_PARSE_FLOW_MAPPING_EMPTY_VALUE_STATE);
            return yaml_parser_parse_node(parser, false, false);
        }
    }
    let event = yaml_event_t {
        data: YamlEventData::MappingEnd,
        start_mark: token.start_mark,
        end_mark: token.end_mark,
    };
    parser.state = parser.states.pop().unwrap();
    _ = parser.marks.pop();
    SKIP_TOKEN(parser);
    Ok(event)
}

fn yaml_parser_parse_flow_mapping_value(
    parser: &mut yaml_parser_t,
    empty: bool,
) -> Result<yaml_event_t, ParserError> {
    let mut token = PEEK_TOKEN(parser)?;
    if empty {
        let mark = token.start_mark;
        parser.state = YAML_PARSE_FLOW_MAPPING_KEY_STATE;
        return yaml_parser_process_empty_scalar(mark);
    }
    if token.data.is_value() {
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser)?;
        if !token.data.is_flow_entry() && !token.data.is_flow_mapping_end() {
            parser.states.push(YAML_PARSE_FLOW_MAPPING_KEY_STATE);
            return yaml_parser_parse_node(parser, false, false);
        }
    }
    let mark = token.start_mark;
    parser.state = YAML_PARSE_FLOW_MAPPING_KEY_STATE;
    yaml_parser_process_empty_scalar(mark)
}

fn yaml_parser_process_empty_scalar(mark: yaml_mark_t) -> Result<yaml_event_t, ParserError> {
    Ok(yaml_event_t {
        data: YamlEventData::Scalar {
            anchor: None,
            tag: None,
            value: String::new(),
            plain_implicit: true,
            quoted_implicit: false,
            style: YAML_PLAIN_SCALAR_STYLE,
        },
        start_mark: mark,
        end_mark: mark,
    })
}

fn yaml_parser_process_directives(
    parser: &mut yaml_parser_t,
    version_directive_ref: Option<&mut Option<yaml_version_directive_t>>,
    tag_directives_ref: Option<&mut Vec<yaml_tag_directive_t>>,
) -> Result<(), ParserError> {
    let default_tag_directives: [yaml_tag_directive_t; 2] = [
        // TODO: Get rid of these heap allocations.
        yaml_tag_directive_t {
            handle: String::from("!"),
            prefix: String::from("!"),
        },
        yaml_tag_directive_t {
            handle: String::from("!!"),
            prefix: String::from("tag:yaml.org,2002:"),
        },
    ];
    let mut version_directive: Option<yaml_version_directive_t> = None;

    let mut tag_directives = Vec::with_capacity(16);

    let mut token = PEEK_TOKEN(parser)?;

    loop {
        if !(token.data.is_version_directive() || token.data.is_tag_directive()) {
            break;
        }

        if let YamlTokenData::VersionDirective { major, minor } = &token.data {
            let mark = token.start_mark;
            if version_directive.is_some() {
                return yaml_parser_set_parser_error("found duplicate %YAML directive", mark);
            } else if *major != 1 || *minor != 1 && *minor != 2 {
                return yaml_parser_set_parser_error("found incompatible YAML document", mark);
            } else {
                version_directive = Some(yaml_version_directive_t {
                    major: *major,
                    minor: *minor,
                });
            }
        } else if let YamlTokenData::TagDirective { handle, prefix } = &token.data {
            let value = yaml_tag_directive_t {
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
    parser: &mut yaml_parser_t,
    value: &yaml_tag_directive_t,
    allow_duplicates: bool,
    mark: yaml_mark_t,
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
