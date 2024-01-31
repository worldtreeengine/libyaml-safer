use alloc::string::String;
use alloc::{vec, vec::Vec};

use crate::scanner::yaml_parser_fetch_more_tokens;
use crate::yaml::{YamlEventData, YamlTokenData};
use crate::{
    yaml_event_t, yaml_mark_t, yaml_parser_t, yaml_tag_directive_t, yaml_token_t,
    yaml_version_directive_t, YAML_BLOCK_MAPPING_STYLE, YAML_BLOCK_SEQUENCE_STYLE,
    YAML_FLOW_MAPPING_STYLE, YAML_FLOW_SEQUENCE_STYLE, YAML_NO_ERROR, YAML_PARSER_ERROR,
    YAML_PARSE_BLOCK_MAPPING_FIRST_KEY_STATE, YAML_PARSE_BLOCK_MAPPING_KEY_STATE,
    YAML_PARSE_BLOCK_MAPPING_VALUE_STATE, YAML_PARSE_BLOCK_NODE_OR_INDENTLESS_SEQUENCE_STATE,
    YAML_PARSE_BLOCK_NODE_STATE, YAML_PARSE_BLOCK_SEQUENCE_ENTRY_STATE,
    YAML_PARSE_BLOCK_SEQUENCE_FIRST_ENTRY_STATE, YAML_PARSE_DOCUMENT_CONTENT_STATE,
    YAML_PARSE_DOCUMENT_END_STATE, YAML_PARSE_DOCUMENT_START_STATE, YAML_PARSE_END_STATE,
    YAML_PARSE_FLOW_MAPPING_EMPTY_VALUE_STATE, YAML_PARSE_FLOW_MAPPING_FIRST_KEY_STATE,
    YAML_PARSE_FLOW_MAPPING_KEY_STATE, YAML_PARSE_FLOW_MAPPING_VALUE_STATE,
    YAML_PARSE_FLOW_NODE_STATE, YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_END_STATE,
    YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_KEY_STATE,
    YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_VALUE_STATE, YAML_PARSE_FLOW_SEQUENCE_ENTRY_STATE,
    YAML_PARSE_FLOW_SEQUENCE_FIRST_ENTRY_STATE, YAML_PARSE_IMPLICIT_DOCUMENT_START_STATE,
    YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE, YAML_PARSE_STREAM_START_STATE,
    YAML_PLAIN_SCALAR_STYLE,
};
use core::ptr;

unsafe fn PEEK_TOKEN(parser: &mut yaml_parser_t) -> *mut yaml_token_t {
    if parser.token_available || yaml_parser_fetch_more_tokens(parser).is_ok() {
        parser.tokens.front_mut().unwrap() as *mut _
    } else {
        ptr::null_mut::<yaml_token_t>()
    }
}

unsafe fn SKIP_TOKEN(parser: &mut yaml_parser_t) {
    parser.token_available = false;
    parser.tokens_parsed = parser.tokens_parsed.wrapping_add(1);
    let skipped = parser.tokens.pop_front();
    parser.stream_end_produced = matches!(
        skipped,
        Some(yaml_token_t {
            data: YamlTokenData::StreamEnd,
            ..
        })
    );
}

/// Parse the input stream and produce the next parsing event.
///
/// Call the function subsequently to produce a sequence of events corresponding
/// to the input stream. The initial event has the type YAML_STREAM_START_EVENT
/// while the ending event has the type YAML_STREAM_END_EVENT.
///
/// An application is responsible for freeing any buffers associated with the
/// produced event object using the yaml_event_delete() function.
///
/// An application must not alternate the calls of yaml_parser_parse() with the
/// calls of yaml_parser_scan() or yaml_parser_load(). Doing this will break the
/// parser.
pub unsafe fn yaml_parser_parse(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
) -> Result<(), ()> {
    *event = yaml_event_t::default();
    if parser.stream_end_produced
        || parser.error != YAML_NO_ERROR
        || parser.state == YAML_PARSE_END_STATE
    {
        return Ok(());
    }
    yaml_parser_state_machine(parser, event)
}

fn yaml_parser_set_parser_error(
    parser: &mut yaml_parser_t,
    problem: &'static str,
    problem_mark: yaml_mark_t,
) {
    parser.error = YAML_PARSER_ERROR;
    parser.problem = Some(problem);
    parser.problem_mark = problem_mark;
}

fn yaml_parser_set_parser_error_context(
    parser: &mut yaml_parser_t,
    context: &'static str,
    context_mark: yaml_mark_t,
    problem: &'static str,
    problem_mark: yaml_mark_t,
) {
    parser.error = YAML_PARSER_ERROR;
    parser.context = Some(context);
    parser.context_mark = context_mark;
    parser.problem = Some(problem);
    parser.problem_mark = problem_mark;
}

unsafe fn yaml_parser_state_machine(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
) -> Result<(), ()> {
    match parser.state {
        YAML_PARSE_STREAM_START_STATE => yaml_parser_parse_stream_start(parser, event),
        YAML_PARSE_IMPLICIT_DOCUMENT_START_STATE => {
            yaml_parser_parse_document_start(parser, event, true)
        }
        YAML_PARSE_DOCUMENT_START_STATE => yaml_parser_parse_document_start(parser, event, false),
        YAML_PARSE_DOCUMENT_CONTENT_STATE => yaml_parser_parse_document_content(parser, event),
        YAML_PARSE_DOCUMENT_END_STATE => yaml_parser_parse_document_end(parser, event),
        YAML_PARSE_BLOCK_NODE_STATE => yaml_parser_parse_node(parser, event, true, false),
        YAML_PARSE_BLOCK_NODE_OR_INDENTLESS_SEQUENCE_STATE => {
            yaml_parser_parse_node(parser, event, true, true)
        }
        YAML_PARSE_FLOW_NODE_STATE => yaml_parser_parse_node(parser, event, false, false),
        YAML_PARSE_BLOCK_SEQUENCE_FIRST_ENTRY_STATE => {
            yaml_parser_parse_block_sequence_entry(parser, event, true)
        }
        YAML_PARSE_BLOCK_SEQUENCE_ENTRY_STATE => {
            yaml_parser_parse_block_sequence_entry(parser, event, false)
        }
        YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE => {
            yaml_parser_parse_indentless_sequence_entry(parser, event)
        }
        YAML_PARSE_BLOCK_MAPPING_FIRST_KEY_STATE => {
            yaml_parser_parse_block_mapping_key(parser, event, true)
        }
        YAML_PARSE_BLOCK_MAPPING_KEY_STATE => {
            yaml_parser_parse_block_mapping_key(parser, event, false)
        }
        YAML_PARSE_BLOCK_MAPPING_VALUE_STATE => {
            yaml_parser_parse_block_mapping_value(parser, event)
        }
        YAML_PARSE_FLOW_SEQUENCE_FIRST_ENTRY_STATE => {
            yaml_parser_parse_flow_sequence_entry(parser, event, true)
        }
        YAML_PARSE_FLOW_SEQUENCE_ENTRY_STATE => {
            yaml_parser_parse_flow_sequence_entry(parser, event, false)
        }
        YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_KEY_STATE => {
            yaml_parser_parse_flow_sequence_entry_mapping_key(parser, event)
        }
        YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_VALUE_STATE => {
            yaml_parser_parse_flow_sequence_entry_mapping_value(parser, event)
        }
        YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_END_STATE => {
            yaml_parser_parse_flow_sequence_entry_mapping_end(parser, event)
        }
        YAML_PARSE_FLOW_MAPPING_FIRST_KEY_STATE => {
            yaml_parser_parse_flow_mapping_key(parser, event, true)
        }
        YAML_PARSE_FLOW_MAPPING_KEY_STATE => {
            yaml_parser_parse_flow_mapping_key(parser, event, false)
        }
        YAML_PARSE_FLOW_MAPPING_VALUE_STATE => {
            yaml_parser_parse_flow_mapping_value(parser, event, false)
        }
        YAML_PARSE_FLOW_MAPPING_EMPTY_VALUE_STATE => {
            yaml_parser_parse_flow_mapping_value(parser, event, true)
        }
        _ => Err(()),
    }
}

unsafe fn yaml_parser_parse_stream_start(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
) -> Result<(), ()> {
    let token: *mut yaml_token_t = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    let token = &mut *token;

    if let YamlTokenData::StreamStart { encoding } = &token.data {
        parser.state = YAML_PARSE_IMPLICIT_DOCUMENT_START_STATE;
        *event = yaml_event_t {
            data: YamlEventData::StreamStart {
                encoding: *encoding,
            },
            start_mark: (*token).start_mark,
            end_mark: (*token).end_mark,
        };
        SKIP_TOKEN(parser);
        Ok(())
    } else {
        yaml_parser_set_parser_error(
            parser,
            "did not find expected <stream-start>",
            (*token).start_mark,
        );
        return Err(());
    }
}

unsafe fn yaml_parser_parse_document_start(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
    implicit: bool,
) -> Result<(), ()> {
    let mut token: *mut yaml_token_t;
    let mut version_directive: Option<yaml_version_directive_t> = None;

    let mut tag_directives = vec![];
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if !implicit {
        while let YamlTokenData::DocumentEnd = &(*token).data {
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser);
            if token.is_null() {
                return Err(());
            }
        }
    }
    if implicit
        && !(*token).data.is_version_directive()
        && !(*token).data.is_tag_directive()
        && !(*token).data.is_document_start()
        && !(*token).data.is_stream_end()
    {
        yaml_parser_process_directives(parser, None, None)?;
        parser.states.push(YAML_PARSE_DOCUMENT_END_STATE);
        parser.state = YAML_PARSE_BLOCK_NODE_STATE;
        *event = yaml_event_t {
            data: YamlEventData::DocumentStart {
                version_directive: None,
                tag_directives: vec![],
                implicit: true,
            },
            start_mark: (*token).start_mark,
            end_mark: (*token).end_mark,
        };
        Ok(())
    } else if !(*token).data.is_stream_end() {
        let end_mark: yaml_mark_t;
        let start_mark: yaml_mark_t = (*token).start_mark;
        yaml_parser_process_directives(
            parser,
            Some(&mut version_directive),
            Some(&mut tag_directives),
        )?;
        token = PEEK_TOKEN(parser);
        if !token.is_null() {
            if !(*token).data.is_document_start() {
                yaml_parser_set_parser_error(
                    parser,
                    "did not find expected <document start>",
                    (*token).start_mark,
                );
            } else {
                parser.states.push(YAML_PARSE_DOCUMENT_END_STATE);
                parser.state = YAML_PARSE_DOCUMENT_CONTENT_STATE;
                end_mark = (*token).end_mark;
                *event = yaml_event_t {
                    data: YamlEventData::DocumentStart {
                        version_directive,
                        tag_directives: core::mem::take(&mut tag_directives),
                        implicit: false,
                    },
                    start_mark,
                    end_mark,
                };
                SKIP_TOKEN(parser);
                return Ok(());
            }
        }
        Err(())
    } else {
        parser.state = YAML_PARSE_END_STATE;
        *event = yaml_event_t {
            data: YamlEventData::StreamEnd,
            start_mark: (*token).start_mark,
            end_mark: (*token).end_mark,
        };
        SKIP_TOKEN(parser);
        Ok(())
    }
}

unsafe fn yaml_parser_parse_document_content(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
) -> Result<(), ()> {
    let token: *mut yaml_token_t = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if let YamlTokenData::VersionDirective { .. }
    | YamlTokenData::TagDirective { .. }
    | YamlTokenData::DocumentStart
    | YamlTokenData::DocumentEnd
    | YamlTokenData::StreamEnd = &(*token).data
    {
        parser.state = parser.states.pop().unwrap();
        yaml_parser_process_empty_scalar(event, (*token).start_mark)
    } else {
        yaml_parser_parse_node(parser, event, true, false)
    }
}

unsafe fn yaml_parser_parse_document_end(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
) -> Result<(), ()> {
    let mut end_mark: yaml_mark_t;
    let mut implicit = true;
    let token: *mut yaml_token_t = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    end_mark = (*token).start_mark;
    let start_mark: yaml_mark_t = end_mark;
    if let YamlTokenData::DocumentEnd = &(*token).data {
        end_mark = (*token).end_mark;
        SKIP_TOKEN(parser);
        implicit = false;
    }
    parser.tag_directives.clear();
    parser.state = YAML_PARSE_DOCUMENT_START_STATE;
    *event = yaml_event_t {
        data: YamlEventData::DocumentEnd { implicit },
        start_mark,
        end_mark,
    };
    Ok(())
}

unsafe fn yaml_parser_parse_node(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
    block: bool,
    indentless_sequence: bool,
) -> Result<(), ()> {
    let mut token: *mut yaml_token_t;
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
    let implicit;

    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }

    if let YamlTokenData::Alias { value } = &mut (*token).data {
        parser.state = parser.states.pop().unwrap();
        *event = yaml_event_t {
            data: YamlEventData::Alias {
                anchor: core::mem::take(value),
            },
            start_mark: (*token).start_mark,
            end_mark: (*token).end_mark,
        };
        SKIP_TOKEN(parser);
        return Ok(());
    }

    end_mark = (*token).start_mark;
    start_mark = end_mark;
    if let YamlTokenData::Anchor { value } = &mut (*token).data {
        anchor = Some(core::mem::take(value));
        start_mark = (*token).start_mark;
        end_mark = (*token).end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        } else if let YamlTokenData::Tag { handle, suffix } = &mut (*token).data {
            tag_handle = Some(core::mem::take(handle));
            tag_suffix = Some(core::mem::take(suffix));
            tag_mark = (*token).start_mark;
            end_mark = (*token).end_mark;
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser);
            if token.is_null() {
                return Err(());
            }
        }
    } else if let YamlTokenData::Tag { handle, suffix } = &mut (*token).data {
        tag_handle = Some(core::mem::take(handle));
        tag_suffix = Some(core::mem::take(suffix));
        tag_mark = (*token).start_mark;
        start_mark = tag_mark;
        end_mark = (*token).end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        } else if let YamlTokenData::Anchor { value } = &mut (*token).data {
            anchor = Some(core::mem::take(value));
            end_mark = (*token).end_mark;
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser);
            if token.is_null() {
                return Err(());
            }
        }
    }

    if let Some(ref tag_handle_value) = tag_handle {
        if tag_handle_value.is_empty() {
            tag = tag_suffix;
        } else {
            for tag_directive in parser.tag_directives.iter() {
                if tag_directive.handle == *tag_handle_value {
                    let suffix = tag_suffix.as_deref().unwrap_or("");
                    tag = Some(alloc::format!("{}{}", tag_directive.prefix, suffix));
                    break;
                }
            }
            if tag.is_none() {
                yaml_parser_set_parser_error_context(
                    parser,
                    "while parsing a node",
                    start_mark,
                    "found undefined tag handle",
                    tag_mark,
                );
                return Err(());
            }
        }
    }

    implicit = tag.is_none() || tag.as_deref() == Some("");

    if indentless_sequence && (*token).data.is_block_entry() {
        end_mark = (*token).end_mark;
        parser.state = YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE;
        *event = yaml_event_t {
            data: YamlEventData::SequenceStart {
                anchor,
                tag,
                implicit,
                style: YAML_BLOCK_SEQUENCE_STYLE,
            },
            start_mark,
            end_mark,
        };
        return Ok(());
    } else if let YamlTokenData::Scalar { value, style } = &mut (*token).data {
        let mut plain_implicit = false;
        let mut quoted_implicit = false;
        end_mark = (*token).end_mark;
        if *style == YAML_PLAIN_SCALAR_STYLE && tag.is_none() || tag.as_deref() == Some("!") {
            plain_implicit = true;
        } else if tag.is_none() {
            quoted_implicit = true;
        }
        parser.state = parser.states.pop().unwrap();
        *event = yaml_event_t {
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
        SKIP_TOKEN(parser);
        return Ok(());
    } else if let YamlTokenData::FlowSequenceStart = &(*token).data {
        end_mark = (*token).end_mark;
        parser.state = YAML_PARSE_FLOW_SEQUENCE_FIRST_ENTRY_STATE;
        *event = yaml_event_t {
            data: YamlEventData::SequenceStart {
                anchor,
                tag,
                implicit,
                style: YAML_FLOW_SEQUENCE_STYLE,
            },
            start_mark,
            end_mark,
        };
        return Ok(());
    } else if let YamlTokenData::FlowMappingStart = &(*token).data {
        end_mark = (*token).end_mark;
        parser.state = YAML_PARSE_FLOW_MAPPING_FIRST_KEY_STATE;
        *event = yaml_event_t {
            data: YamlEventData::MappingStart {
                anchor,
                tag,
                implicit,
                style: YAML_FLOW_MAPPING_STYLE,
            },
            start_mark,
            end_mark,
        };
        return Ok(());
    } else if block && (*token).data.is_block_sequence_start() {
        end_mark = (*token).end_mark;
        parser.state = YAML_PARSE_BLOCK_SEQUENCE_FIRST_ENTRY_STATE;
        *event = yaml_event_t {
            data: YamlEventData::SequenceStart {
                anchor,
                tag,
                implicit,
                style: YAML_BLOCK_SEQUENCE_STYLE,
            },
            start_mark,
            end_mark,
        };
        return Ok(());
    } else if block && (*token).data.is_block_mapping_start() {
        end_mark = (*token).end_mark;
        parser.state = YAML_PARSE_BLOCK_MAPPING_FIRST_KEY_STATE;
        *event = yaml_event_t {
            data: YamlEventData::MappingStart {
                anchor,
                tag,
                implicit,
                style: YAML_BLOCK_MAPPING_STYLE,
            },
            start_mark,
            end_mark,
        };
        return Ok(());
    } else if anchor.is_some() || tag.is_some() {
        parser.state = parser.states.pop().unwrap();
        *event = yaml_event_t {
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
        return Ok(());
    } else {
        yaml_parser_set_parser_error_context(
            parser,
            if block {
                "while parsing a block node"
            } else {
                "while parsing a flow node"
            },
            start_mark,
            "did not find expected node content",
            (*token).start_mark,
        );
        Err(())
    }
}

unsafe fn yaml_parser_parse_block_sequence_entry(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
    first: bool,
) -> Result<(), ()> {
    let mut token: *mut yaml_token_t;
    if first {
        token = PEEK_TOKEN(parser);
        parser.marks.push((*token).start_mark);
        SKIP_TOKEN(parser);
    }
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if let YamlTokenData::BlockEntry = &(*token).data {
        let mark: yaml_mark_t = (*token).end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if !(*token).data.is_block_entry() && !(*token).data.is_block_end() {
            parser.states.push(YAML_PARSE_BLOCK_SEQUENCE_ENTRY_STATE);
            yaml_parser_parse_node(parser, event, true, false)
        } else {
            parser.state = YAML_PARSE_BLOCK_SEQUENCE_ENTRY_STATE;
            yaml_parser_process_empty_scalar(event, mark)
        }
    } else if (*token).data.is_block_end() {
        parser.state = parser.states.pop().unwrap();
        let _ = parser.marks.pop();
        *event = yaml_event_t {
            data: YamlEventData::SequenceEnd,
            start_mark: (*token).start_mark,
            end_mark: (*token).end_mark,
        };
        SKIP_TOKEN(parser);
        Ok(())
    } else {
        let mark = parser.marks.pop().unwrap();
        yaml_parser_set_parser_error_context(
            parser,
            "while parsing a block collection",
            mark,
            "did not find expected '-' indicator",
            (*token).start_mark,
        );
        Err(())
    }
}

unsafe fn yaml_parser_parse_indentless_sequence_entry(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
) -> Result<(), ()> {
    let mut token: *mut yaml_token_t;
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if (*token).data.is_block_entry() {
        let mark: yaml_mark_t = (*token).end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if !(*token).data.is_block_entry()
            && !(*token).data.is_key()
            && !(*token).data.is_value()
            && !(*token).data.is_block_end()
        {
            parser
                .states
                .push(YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE);
            yaml_parser_parse_node(parser, event, true, false)
        } else {
            parser.state = YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE;
            yaml_parser_process_empty_scalar(event, mark)
        }
    } else {
        parser.state = parser.states.pop().unwrap();
        *event = yaml_event_t {
            data: YamlEventData::SequenceEnd,
            start_mark: (*token).start_mark,
            end_mark: (*token).end_mark,
        };
        Ok(())
    }
}

unsafe fn yaml_parser_parse_block_mapping_key(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
    first: bool,
) -> Result<(), ()> {
    let mut token: *mut yaml_token_t;
    if first {
        token = PEEK_TOKEN(parser);
        parser.marks.push((*token).start_mark);
        SKIP_TOKEN(parser);
    }
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if (*token).data.is_key() {
        let mark: yaml_mark_t = (*token).end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if !(*token).data.is_key() && !(*token).data.is_value() && !(*token).data.is_block_end() {
            parser.states.push(YAML_PARSE_BLOCK_MAPPING_VALUE_STATE);
            yaml_parser_parse_node(parser, event, true, true)
        } else {
            parser.state = YAML_PARSE_BLOCK_MAPPING_VALUE_STATE;
            yaml_parser_process_empty_scalar(event, mark)
        }
    } else if (*token).data.is_block_end() {
        parser.state = parser.states.pop().unwrap();
        _ = parser.marks.pop();
        *event = yaml_event_t {
            data: YamlEventData::MappingEnd,
            start_mark: (*token).start_mark,
            end_mark: (*token).end_mark,
        };
        SKIP_TOKEN(parser);
        Ok(())
    } else {
        let mark = parser.marks.pop().unwrap();
        yaml_parser_set_parser_error_context(
            parser,
            "while parsing a block mapping",
            mark,
            "did not find expected key",
            (*token).start_mark,
        );
        Err(())
    }
}

unsafe fn yaml_parser_parse_block_mapping_value(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
) -> Result<(), ()> {
    let mut token: *mut yaml_token_t;
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if (*token).data.is_value() {
        let mark: yaml_mark_t = (*token).end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if !(*token).data.is_key() && !(*token).data.is_value() && !(*token).data.is_block_end() {
            parser.states.push(YAML_PARSE_BLOCK_MAPPING_KEY_STATE);
            yaml_parser_parse_node(parser, event, true, true)
        } else {
            parser.state = YAML_PARSE_BLOCK_MAPPING_KEY_STATE;
            yaml_parser_process_empty_scalar(event, mark)
        }
    } else {
        parser.state = YAML_PARSE_BLOCK_MAPPING_KEY_STATE;
        yaml_parser_process_empty_scalar(event, (*token).start_mark)
    }
}

unsafe fn yaml_parser_parse_flow_sequence_entry(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
    first: bool,
) -> Result<(), ()> {
    let mut token: *mut yaml_token_t;
    if first {
        token = PEEK_TOKEN(parser);
        parser.marks.push((*token).start_mark);
        SKIP_TOKEN(parser);
    }
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if !(*token).data.is_flow_sequence_end() {
        if !first {
            if (*token).data.is_flow_entry() {
                SKIP_TOKEN(parser);
                token = PEEK_TOKEN(parser);
                if token.is_null() {
                    return Err(());
                }
            } else {
                let mark = parser.marks.pop().unwrap();
                yaml_parser_set_parser_error_context(
                    parser,
                    "while parsing a flow sequence",
                    mark,
                    "did not find expected ',' or ']'",
                    (*token).start_mark,
                );
                return Err(());
            }
        }
        if (*token).data.is_key() {
            parser.state = YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_KEY_STATE;
            *event = yaml_event_t::default();
            *event = yaml_event_t {
                data: YamlEventData::MappingStart {
                    anchor: None,
                    tag: None,
                    implicit: true,
                    style: YAML_FLOW_MAPPING_STYLE,
                },
                start_mark: (*token).start_mark,
                end_mark: (*token).end_mark,
            };
            SKIP_TOKEN(parser);
            return Ok(());
        } else if !(*token).data.is_flow_sequence_end() {
            parser.states.push(YAML_PARSE_FLOW_SEQUENCE_ENTRY_STATE);
            return yaml_parser_parse_node(parser, event, false, false);
        }
    }
    parser.state = parser.states.pop().unwrap();
    _ = parser.marks.pop();
    *event = yaml_event_t {
        data: YamlEventData::SequenceEnd,
        start_mark: (*token).start_mark,
        end_mark: (*token).end_mark,
    };
    SKIP_TOKEN(parser);
    Ok(())
}

unsafe fn yaml_parser_parse_flow_sequence_entry_mapping_key(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
) -> Result<(), ()> {
    let token: *mut yaml_token_t = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if !(*token).data.is_value()
        && !(*token).data.is_flow_entry()
        && !(*token).data.is_flow_sequence_end()
    {
        parser
            .states
            .push(YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_VALUE_STATE);
        yaml_parser_parse_node(parser, event, false, false)
    } else {
        let mark: yaml_mark_t = (*token).end_mark;
        SKIP_TOKEN(parser);
        parser.state = YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_VALUE_STATE;
        yaml_parser_process_empty_scalar(event, mark)
    }
}

unsafe fn yaml_parser_parse_flow_sequence_entry_mapping_value(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
) -> Result<(), ()> {
    let mut token: *mut yaml_token_t;
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if (*token).data.is_value() {
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if !(*token).data.is_flow_entry() && !(*token).data.is_flow_sequence_end() {
            parser
                .states
                .push(YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_END_STATE);
            return yaml_parser_parse_node(parser, event, false, false);
        }
    }
    parser.state = YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_END_STATE;
    yaml_parser_process_empty_scalar(event, (*token).start_mark)
}

unsafe fn yaml_parser_parse_flow_sequence_entry_mapping_end(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
) -> Result<(), ()> {
    let token: *mut yaml_token_t = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    parser.state = YAML_PARSE_FLOW_SEQUENCE_ENTRY_STATE;
    *event = yaml_event_t {
        data: YamlEventData::MappingEnd,
        start_mark: (*token).start_mark,
        end_mark: (*token).end_mark,
    };
    Ok(())
}

unsafe fn yaml_parser_parse_flow_mapping_key(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
    first: bool,
) -> Result<(), ()> {
    let mut token: *mut yaml_token_t;
    if first {
        token = PEEK_TOKEN(parser);
        parser.marks.push((*token).start_mark);
        SKIP_TOKEN(parser);
    }
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if !(*token).data.is_flow_mapping_end() {
        if !first {
            if (*token).data.is_flow_entry() {
                SKIP_TOKEN(parser);
                token = PEEK_TOKEN(parser);
                if token.is_null() {
                    return Err(());
                }
            } else {
                let mark = parser.marks.pop().unwrap();
                yaml_parser_set_parser_error_context(
                    parser,
                    "while parsing a flow mapping",
                    mark,
                    "did not find expected ',' or '}'",
                    (*token).start_mark,
                );
                return Err(());
            }
        }
        if (*token).data.is_key() {
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser);
            if token.is_null() {
                return Err(());
            }
            if !(*token).data.is_value()
                && !(*token).data.is_flow_entry()
                && !(*token).data.is_flow_mapping_end()
            {
                parser.states.push(YAML_PARSE_FLOW_MAPPING_VALUE_STATE);
                return yaml_parser_parse_node(parser, event, false, false);
            } else {
                parser.state = YAML_PARSE_FLOW_MAPPING_VALUE_STATE;
                return yaml_parser_process_empty_scalar(event, (*token).start_mark);
            }
        } else if !(*token).data.is_flow_mapping_end() {
            parser
                .states
                .push(YAML_PARSE_FLOW_MAPPING_EMPTY_VALUE_STATE);
            return yaml_parser_parse_node(parser, event, false, false);
        }
    }
    parser.state = parser.states.pop().unwrap();
    _ = parser.marks.pop();
    *event = yaml_event_t {
        data: YamlEventData::MappingEnd,
        start_mark: (*token).start_mark,
        end_mark: (*token).end_mark,
    };
    SKIP_TOKEN(parser);
    Ok(())
}

unsafe fn yaml_parser_parse_flow_mapping_value(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
    empty: bool,
) -> Result<(), ()> {
    let mut token: *mut yaml_token_t;
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if empty {
        parser.state = YAML_PARSE_FLOW_MAPPING_KEY_STATE;
        return yaml_parser_process_empty_scalar(event, (*token).start_mark);
    }
    if (*token).data.is_value() {
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if !(*token).data.is_flow_entry() && !(*token).data.is_flow_mapping_end() {
            parser.states.push(YAML_PARSE_FLOW_MAPPING_KEY_STATE);
            return yaml_parser_parse_node(parser, event, false, false);
        }
    }
    parser.state = YAML_PARSE_FLOW_MAPPING_KEY_STATE;
    yaml_parser_process_empty_scalar(event, (*token).start_mark)
}

unsafe fn yaml_parser_process_empty_scalar(
    event: &mut yaml_event_t,
    mark: yaml_mark_t,
) -> Result<(), ()> {
    *event = yaml_event_t {
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
    };
    Ok(())
}

unsafe fn yaml_parser_process_directives(
    parser: &mut yaml_parser_t,
    version_directive_ref: Option<&mut Option<yaml_version_directive_t>>,
    tag_directives_ref: Option<&mut Vec<yaml_tag_directive_t>>,
) -> Result<(), ()> {
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

    let mut token: *mut yaml_token_t;
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }

    loop {
        if !((*token).data.is_version_directive() || (*token).data.is_tag_directive()) {
            break;
        }

        if let YamlTokenData::VersionDirective { major, minor } = &(*token).data {
            if version_directive.is_some() {
                yaml_parser_set_parser_error(
                    parser,
                    "found duplicate %YAML directive",
                    (*token).start_mark,
                );
                return Err(())?;
            } else if *major != 1 || *minor != 1 && *minor != 2 {
                yaml_parser_set_parser_error(
                    parser,
                    "found incompatible YAML document",
                    (*token).start_mark,
                );
                return Err(());
            } else {
                version_directive = Some(yaml_version_directive_t {
                    major: *major,
                    minor: *minor,
                });
            }
        } else if let YamlTokenData::TagDirective { handle, prefix } = &(*token).data {
            let value = yaml_tag_directive_t {
                // TODO: Get rid of these clones by consuming tokens by value.
                handle: handle.clone(),
                prefix: prefix.clone(),
            };
            yaml_parser_append_tag_directive(parser, &value, false, (*token).start_mark)?;

            tag_directives.push(value);
        }

        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
    }

    for default_tag_directive in &default_tag_directives {
        yaml_parser_append_tag_directive(parser, default_tag_directive, true, (*token).start_mark)?
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

unsafe fn yaml_parser_append_tag_directive(
    parser: &mut yaml_parser_t,
    value: &yaml_tag_directive_t,
    allow_duplicates: bool,
    mark: yaml_mark_t,
) -> Result<(), ()> {
    for tag_directive in parser.tag_directives.iter() {
        if value.handle == tag_directive.handle {
            if allow_duplicates {
                return Ok(());
            }
            yaml_parser_set_parser_error(parser, "found duplicate %TAG directive", mark);
            return Err(());
        }
    }
    parser.tag_directives.push(value.clone());
    Ok(())
}
