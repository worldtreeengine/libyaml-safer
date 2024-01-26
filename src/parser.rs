use crate::api::{yaml_free, yaml_malloc, yaml_stack_extend, yaml_strdup};
use crate::externs::{memcpy, strcmp, strlen};
use crate::ops::ForceAdd as _;
use crate::scanner::yaml_parser_fetch_more_tokens;
use crate::yaml::{size_t, yaml_char_t};
use crate::{
    libc, yaml_event_t, yaml_mark_t, yaml_parser_t, yaml_tag_directive_t, yaml_token_t,
    yaml_version_directive_t, YAML_ALIAS_EVENT, YAML_ALIAS_TOKEN, YAML_ANCHOR_TOKEN,
    YAML_BLOCK_END_TOKEN, YAML_BLOCK_ENTRY_TOKEN, YAML_BLOCK_MAPPING_START_TOKEN,
    YAML_BLOCK_MAPPING_STYLE, YAML_BLOCK_SEQUENCE_START_TOKEN, YAML_BLOCK_SEQUENCE_STYLE,
    YAML_DOCUMENT_END_EVENT, YAML_DOCUMENT_END_TOKEN, YAML_DOCUMENT_START_EVENT,
    YAML_DOCUMENT_START_TOKEN, YAML_FLOW_ENTRY_TOKEN, YAML_FLOW_MAPPING_END_TOKEN,
    YAML_FLOW_MAPPING_START_TOKEN, YAML_FLOW_MAPPING_STYLE, YAML_FLOW_SEQUENCE_END_TOKEN,
    YAML_FLOW_SEQUENCE_START_TOKEN, YAML_FLOW_SEQUENCE_STYLE, YAML_KEY_TOKEN,
    YAML_MAPPING_END_EVENT, YAML_MAPPING_START_EVENT, YAML_NO_ERROR, YAML_PARSER_ERROR,
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
    YAML_PLAIN_SCALAR_STYLE, YAML_SCALAR_EVENT, YAML_SCALAR_TOKEN, YAML_SEQUENCE_END_EVENT,
    YAML_SEQUENCE_START_EVENT, YAML_STREAM_END_EVENT, YAML_STREAM_END_TOKEN,
    YAML_STREAM_START_EVENT, YAML_STREAM_START_TOKEN, YAML_TAG_DIRECTIVE_TOKEN, YAML_TAG_TOKEN,
    YAML_VALUE_TOKEN, YAML_VERSION_DIRECTIVE_TOKEN,
};
use core::mem::size_of;
use core::ptr::{self, addr_of_mut};

unsafe fn PEEK_TOKEN(parser: &mut yaml_parser_t) -> *mut yaml_token_t {
    if parser.token_available || yaml_parser_fetch_more_tokens(parser).is_ok() {
        parser.tokens.head
    } else {
        ptr::null_mut::<yaml_token_t>()
    }
}

unsafe fn SKIP_TOKEN(parser: &mut yaml_parser_t) {
    parser.token_available = false;
    parser.tokens_parsed = parser.tokens_parsed.wrapping_add(1);
    parser.stream_end_produced = (*parser.tokens.head).type_ == YAML_STREAM_END_TOKEN;
    parser.tokens.head = parser.tokens.head.wrapping_offset(1);
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

unsafe fn yaml_parser_set_parser_error(
    parser: &mut yaml_parser_t,
    problem: &'static str,
    problem_mark: yaml_mark_t,
) {
    parser.error = YAML_PARSER_ERROR;
    parser.problem = Some(problem);
    parser.problem_mark = problem_mark;
}

unsafe fn yaml_parser_set_parser_error_context(
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
    if (*token).type_ != YAML_STREAM_START_TOKEN {
        yaml_parser_set_parser_error(
            parser,
            "did not find expected <stream-start>",
            (*token).start_mark,
        );
        return Err(());
    }
    parser.state = YAML_PARSE_IMPLICIT_DOCUMENT_START_STATE;
    *event = yaml_event_t::default();
    event.type_ = YAML_STREAM_START_EVENT;
    event.start_mark = (*token).start_mark;
    event.end_mark = (*token).start_mark;
    event.data.stream_start.encoding = (*token).data.stream_start.encoding;
    SKIP_TOKEN(parser);
    Ok(())
}

unsafe fn yaml_parser_parse_document_start(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
    implicit: bool,
) -> Result<(), ()> {
    let mut token: *mut yaml_token_t;
    let mut version_directive: *mut yaml_version_directive_t =
        ptr::null_mut::<yaml_version_directive_t>();
    struct TagDirectives {
        start: *mut yaml_tag_directive_t,
        end: *mut yaml_tag_directive_t,
    }
    let mut tag_directives = TagDirectives {
        start: ptr::null_mut::<yaml_tag_directive_t>(),
        end: ptr::null_mut::<yaml_tag_directive_t>(),
    };
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if !implicit {
        while (*token).type_ == YAML_DOCUMENT_END_TOKEN {
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser);
            if token.is_null() {
                return Err(());
            }
        }
    }
    if implicit
        && (*token).type_ != YAML_VERSION_DIRECTIVE_TOKEN
        && (*token).type_ != YAML_TAG_DIRECTIVE_TOKEN
        && (*token).type_ != YAML_DOCUMENT_START_TOKEN
        && (*token).type_ != YAML_STREAM_END_TOKEN
    {
        yaml_parser_process_directives(
            parser,
            ptr::null_mut::<*mut yaml_version_directive_t>(),
            ptr::null_mut::<*mut yaml_tag_directive_t>(),
            ptr::null_mut::<*mut yaml_tag_directive_t>(),
        )?;
        PUSH!(parser.states, YAML_PARSE_DOCUMENT_END_STATE);
        parser.state = YAML_PARSE_BLOCK_NODE_STATE;
        *event = yaml_event_t::default();
        event.type_ = YAML_DOCUMENT_START_EVENT;
        event.start_mark = (*token).start_mark;
        event.end_mark = (*token).start_mark;
        event.data.document_start.version_directive = ptr::null_mut();
        event.data.document_start.tag_directives.start = ptr::null_mut();
        event.data.document_start.tag_directives.end = ptr::null_mut();
        event.data.document_start.implicit = true;
        Ok(())
    } else if (*token).type_ != YAML_STREAM_END_TOKEN {
        let end_mark: yaml_mark_t;
        let start_mark: yaml_mark_t = (*token).start_mark;
        yaml_parser_process_directives(
            parser,
            addr_of_mut!(version_directive),
            addr_of_mut!(tag_directives.start),
            addr_of_mut!(tag_directives.end),
        )?;
        token = PEEK_TOKEN(parser);
        if !token.is_null() {
            if (*token).type_ != YAML_DOCUMENT_START_TOKEN {
                yaml_parser_set_parser_error(
                    parser,
                    "did not find expected <document start>",
                    (*token).start_mark,
                );
            } else {
                PUSH!(parser.states, YAML_PARSE_DOCUMENT_END_STATE);
                parser.state = YAML_PARSE_DOCUMENT_CONTENT_STATE;
                end_mark = (*token).end_mark;
                *event = yaml_event_t::default();
                event.type_ = YAML_DOCUMENT_START_EVENT;
                event.start_mark = start_mark;
                event.end_mark = end_mark;
                event.data.document_start.version_directive = version_directive;
                event.data.document_start.tag_directives.start = tag_directives.start;
                event.data.document_start.tag_directives.end = tag_directives.end;
                event.data.document_start.implicit = false;
                SKIP_TOKEN(parser);
                tag_directives.end = ptr::null_mut::<yaml_tag_directive_t>();
                tag_directives.start = tag_directives.end;
                return Ok(());
            }
        }
        yaml_free(version_directive as *mut libc::c_void);
        while tag_directives.start != tag_directives.end {
            yaml_free((*tag_directives.end.wrapping_offset(-1_isize)).handle as *mut libc::c_void);
            yaml_free((*tag_directives.end.wrapping_offset(-1_isize)).prefix as *mut libc::c_void);
            tag_directives.end = tag_directives.end.wrapping_offset(-1);
        }
        yaml_free(tag_directives.start as *mut libc::c_void);
        Err(())
    } else {
        parser.state = YAML_PARSE_END_STATE;
        *event = yaml_event_t::default();
        event.type_ = YAML_STREAM_END_EVENT;
        event.start_mark = (*token).start_mark;
        event.end_mark = (*token).end_mark;
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
    if (*token).type_ == YAML_VERSION_DIRECTIVE_TOKEN
        || (*token).type_ == YAML_TAG_DIRECTIVE_TOKEN
        || (*token).type_ == YAML_DOCUMENT_START_TOKEN
        || (*token).type_ == YAML_DOCUMENT_END_TOKEN
        || (*token).type_ == YAML_STREAM_END_TOKEN
    {
        parser.state = POP!(parser.states);
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
    if (*token).type_ == YAML_DOCUMENT_END_TOKEN {
        end_mark = (*token).end_mark;
        SKIP_TOKEN(parser);
        implicit = false;
    }
    while !STACK_EMPTY!(parser.tag_directives) {
        let tag_directive = POP!(parser.tag_directives);
        yaml_free(tag_directive.handle as *mut libc::c_void);
        yaml_free(tag_directive.prefix as *mut libc::c_void);
    }
    parser.state = YAML_PARSE_DOCUMENT_START_STATE;
    *event = yaml_event_t::default();
    event.type_ = YAML_DOCUMENT_END_EVENT;
    event.start_mark = start_mark;
    event.end_mark = end_mark;
    event.data.document_end.implicit = implicit;
    Ok(())
}

unsafe fn yaml_parser_parse_node(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
    block: bool,
    indentless_sequence: bool,
) -> Result<(), ()> {
    let mut current_block: u64;
    let mut token: *mut yaml_token_t;
    let mut anchor: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    let mut tag_handle: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    let mut tag_suffix: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    let mut tag: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
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
    if (*token).type_ == YAML_ALIAS_TOKEN {
        parser.state = POP!(parser.states);
        *event = yaml_event_t::default();
        event.type_ = YAML_ALIAS_EVENT;
        event.start_mark = (*token).start_mark;
        event.end_mark = (*token).end_mark;
        event.data.alias.anchor = (*token).data.alias.value;
        SKIP_TOKEN(parser);
        Ok(())
    } else {
        end_mark = (*token).start_mark;
        start_mark = end_mark;
        if (*token).type_ == YAML_ANCHOR_TOKEN {
            anchor = (*token).data.anchor.value;
            start_mark = (*token).start_mark;
            end_mark = (*token).end_mark;
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser);
            if token.is_null() {
                current_block = 17786380918591080555;
            } else if (*token).type_ == YAML_TAG_TOKEN {
                tag_handle = (*token).data.tag.handle;
                tag_suffix = (*token).data.tag.suffix;
                tag_mark = (*token).start_mark;
                end_mark = (*token).end_mark;
                SKIP_TOKEN(parser);
                token = PEEK_TOKEN(parser);
                if token.is_null() {
                    current_block = 17786380918591080555;
                } else {
                    current_block = 11743904203796629665;
                }
            } else {
                current_block = 11743904203796629665;
            }
        } else if (*token).type_ == YAML_TAG_TOKEN {
            tag_handle = (*token).data.tag.handle;
            tag_suffix = (*token).data.tag.suffix;
            tag_mark = (*token).start_mark;
            start_mark = tag_mark;
            end_mark = (*token).end_mark;
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser);
            if token.is_null() {
                current_block = 17786380918591080555;
            } else if (*token).type_ == YAML_ANCHOR_TOKEN {
                anchor = (*token).data.anchor.value;
                end_mark = (*token).end_mark;
                SKIP_TOKEN(parser);
                token = PEEK_TOKEN(parser);
                if token.is_null() {
                    current_block = 17786380918591080555;
                } else {
                    current_block = 11743904203796629665;
                }
            } else {
                current_block = 11743904203796629665;
            }
        } else {
            current_block = 11743904203796629665;
        }
        if current_block == 11743904203796629665 {
            if !tag_handle.is_null() {
                if *tag_handle == 0 {
                    tag = tag_suffix;
                    yaml_free(tag_handle as *mut libc::c_void);
                    tag_suffix = ptr::null_mut::<yaml_char_t>();
                    tag_handle = tag_suffix;
                    current_block = 9437013279121998969;
                } else {
                    let mut tag_directive: *mut yaml_tag_directive_t;
                    tag_directive = parser.tag_directives.start;
                    loop {
                        if !(tag_directive != parser.tag_directives.top) {
                            current_block = 17728966195399430138;
                            break;
                        }
                        if strcmp(
                            (*tag_directive).handle as *mut libc::c_char,
                            tag_handle as *mut libc::c_char,
                        ) == 0
                        {
                            let prefix_len: size_t =
                                strlen((*tag_directive).prefix as *mut libc::c_char);
                            let suffix_len: size_t = strlen(tag_suffix as *mut libc::c_char);
                            tag = yaml_malloc(prefix_len.force_add(suffix_len).force_add(1_u64))
                                as *mut yaml_char_t;
                            memcpy(
                                tag as *mut libc::c_void,
                                (*tag_directive).prefix as *const libc::c_void,
                                prefix_len,
                            );
                            memcpy(
                                tag.wrapping_offset(prefix_len as isize) as *mut libc::c_void,
                                tag_suffix as *const libc::c_void,
                                suffix_len,
                            );
                            *tag.wrapping_offset(prefix_len.force_add(suffix_len) as isize) = b'\0';
                            yaml_free(tag_handle as *mut libc::c_void);
                            yaml_free(tag_suffix as *mut libc::c_void);
                            tag_suffix = ptr::null_mut::<yaml_char_t>();
                            tag_handle = tag_suffix;
                            current_block = 17728966195399430138;
                            break;
                        } else {
                            tag_directive = tag_directive.wrapping_offset(1);
                        }
                    }
                    if current_block != 17786380918591080555 {
                        if tag.is_null() {
                            yaml_parser_set_parser_error_context(
                                parser,
                                "while parsing a node",
                                start_mark,
                                "found undefined tag handle",
                                tag_mark,
                            );
                            current_block = 17786380918591080555;
                        } else {
                            current_block = 9437013279121998969;
                        }
                    }
                }
            } else {
                current_block = 9437013279121998969;
            }
            if current_block != 17786380918591080555 {
                implicit = tag.is_null() || *tag == 0;
                if indentless_sequence && (*token).type_ == YAML_BLOCK_ENTRY_TOKEN {
                    end_mark = (*token).end_mark;
                    parser.state = YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE;
                    *event = yaml_event_t::default();
                    event.type_ = YAML_SEQUENCE_START_EVENT;
                    event.start_mark = start_mark;
                    event.end_mark = end_mark;
                    event.data.sequence_start.anchor = anchor;
                    event.data.sequence_start.tag = tag;
                    event.data.sequence_start.implicit = implicit;
                    event.data.sequence_start.style = YAML_BLOCK_SEQUENCE_STYLE;
                    return Ok(());
                } else if (*token).type_ == YAML_SCALAR_TOKEN {
                    let mut plain_implicit = false;
                    let mut quoted_implicit = false;
                    end_mark = (*token).end_mark;
                    if (*token).data.scalar.style == YAML_PLAIN_SCALAR_STYLE && tag.is_null()
                        || !tag.is_null()
                            && strcmp(
                                tag as *mut libc::c_char,
                                b"!\0" as *const u8 as *const libc::c_char,
                            ) == 0
                    {
                        plain_implicit = true;
                    } else if tag.is_null() {
                        quoted_implicit = true;
                    }
                    parser.state = POP!(parser.states);
                    *event = yaml_event_t::default();
                    event.type_ = YAML_SCALAR_EVENT;
                    event.start_mark = start_mark;
                    event.end_mark = end_mark;
                    event.data.scalar.anchor = anchor;
                    event.data.scalar.tag = tag;
                    event.data.scalar.value = (*token).data.scalar.value;
                    event.data.scalar.length = (*token).data.scalar.length;
                    event.data.scalar.plain_implicit = plain_implicit;
                    event.data.scalar.quoted_implicit = quoted_implicit;
                    event.data.scalar.style = (*token).data.scalar.style;
                    SKIP_TOKEN(parser);
                    return Ok(());
                } else if (*token).type_ == YAML_FLOW_SEQUENCE_START_TOKEN {
                    end_mark = (*token).end_mark;
                    parser.state = YAML_PARSE_FLOW_SEQUENCE_FIRST_ENTRY_STATE;
                    *event = yaml_event_t::default();
                    event.type_ = YAML_SEQUENCE_START_EVENT;
                    event.start_mark = start_mark;
                    event.end_mark = end_mark;
                    event.data.sequence_start.anchor = anchor;
                    event.data.sequence_start.tag = tag;
                    event.data.sequence_start.implicit = implicit;
                    event.data.sequence_start.style = YAML_FLOW_SEQUENCE_STYLE;
                    return Ok(());
                } else if (*token).type_ == YAML_FLOW_MAPPING_START_TOKEN {
                    end_mark = (*token).end_mark;
                    parser.state = YAML_PARSE_FLOW_MAPPING_FIRST_KEY_STATE;
                    *event = yaml_event_t::default();
                    event.type_ = YAML_MAPPING_START_EVENT;
                    event.start_mark = start_mark;
                    event.end_mark = end_mark;
                    event.data.mapping_start.anchor = anchor;
                    event.data.mapping_start.tag = tag;
                    event.data.mapping_start.implicit = implicit;
                    event.data.mapping_start.style = YAML_FLOW_MAPPING_STYLE;
                    return Ok(());
                } else if block && (*token).type_ == YAML_BLOCK_SEQUENCE_START_TOKEN {
                    end_mark = (*token).end_mark;
                    parser.state = YAML_PARSE_BLOCK_SEQUENCE_FIRST_ENTRY_STATE;
                    *event = yaml_event_t::default();
                    event.type_ = YAML_SEQUENCE_START_EVENT;
                    event.start_mark = start_mark;
                    event.end_mark = end_mark;
                    event.data.sequence_start.anchor = anchor;
                    event.data.sequence_start.tag = tag;
                    event.data.sequence_start.implicit = implicit;
                    event.data.sequence_start.style = YAML_BLOCK_SEQUENCE_STYLE;
                    return Ok(());
                } else if block && (*token).type_ == YAML_BLOCK_MAPPING_START_TOKEN {
                    end_mark = (*token).end_mark;
                    parser.state = YAML_PARSE_BLOCK_MAPPING_FIRST_KEY_STATE;
                    *event = yaml_event_t::default();
                    event.type_ = YAML_MAPPING_START_EVENT;
                    event.start_mark = start_mark;
                    event.end_mark = end_mark;
                    event.data.mapping_start.anchor = anchor;
                    event.data.mapping_start.tag = tag;
                    event.data.mapping_start.implicit = implicit;
                    event.data.mapping_start.style = YAML_BLOCK_MAPPING_STYLE;
                    return Ok(());
                } else if !anchor.is_null() || !tag.is_null() {
                    let value: *mut yaml_char_t = yaml_malloc(1_u64) as *mut yaml_char_t;
                    *value = b'\0';
                    parser.state = POP!(parser.states);
                    *event = yaml_event_t::default();
                    event.type_ = YAML_SCALAR_EVENT;
                    event.start_mark = start_mark;
                    event.end_mark = end_mark;
                    event.data.scalar.anchor = anchor;
                    event.data.scalar.tag = tag;
                    event.data.scalar.value = value;
                    event.data.scalar.length = 0_u64;
                    event.data.scalar.plain_implicit = implicit;
                    event.data.scalar.quoted_implicit = false;
                    event.data.scalar.style = YAML_PLAIN_SCALAR_STYLE;
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
                }
            }
        }
        yaml_free(anchor as *mut libc::c_void);
        yaml_free(tag_handle as *mut libc::c_void);
        yaml_free(tag_suffix as *mut libc::c_void);
        yaml_free(tag as *mut libc::c_void);
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
        PUSH!(parser.marks, (*token).start_mark);
        SKIP_TOKEN(parser);
    }
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if (*token).type_ == YAML_BLOCK_ENTRY_TOKEN {
        let mark: yaml_mark_t = (*token).end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if (*token).type_ != YAML_BLOCK_ENTRY_TOKEN && (*token).type_ != YAML_BLOCK_END_TOKEN {
            PUSH!(parser.states, YAML_PARSE_BLOCK_SEQUENCE_ENTRY_STATE);
            yaml_parser_parse_node(parser, event, true, false)
        } else {
            parser.state = YAML_PARSE_BLOCK_SEQUENCE_ENTRY_STATE;
            yaml_parser_process_empty_scalar(event, mark)
        }
    } else if (*token).type_ == YAML_BLOCK_END_TOKEN {
        parser.state = POP!(parser.states);
        let _ = POP!(parser.marks);
        *event = yaml_event_t::default();
        event.type_ = YAML_SEQUENCE_END_EVENT;
        event.start_mark = (*token).start_mark;
        event.end_mark = (*token).end_mark;
        SKIP_TOKEN(parser);
        Ok(())
    } else {
        let mark = POP!(parser.marks);
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
    if (*token).type_ == YAML_BLOCK_ENTRY_TOKEN {
        let mark: yaml_mark_t = (*token).end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if (*token).type_ != YAML_BLOCK_ENTRY_TOKEN
            && (*token).type_ != YAML_KEY_TOKEN
            && (*token).type_ != YAML_VALUE_TOKEN
            && (*token).type_ != YAML_BLOCK_END_TOKEN
        {
            PUSH!(parser.states, YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE);
            yaml_parser_parse_node(parser, event, true, false)
        } else {
            parser.state = YAML_PARSE_INDENTLESS_SEQUENCE_ENTRY_STATE;
            yaml_parser_process_empty_scalar(event, mark)
        }
    } else {
        parser.state = POP!(parser.states);
        *event = yaml_event_t::default();
        event.type_ = YAML_SEQUENCE_END_EVENT;
        event.start_mark = (*token).start_mark;
        event.end_mark = (*token).start_mark;
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
        PUSH!(parser.marks, (*token).start_mark);
        SKIP_TOKEN(parser);
    }
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if (*token).type_ == YAML_KEY_TOKEN {
        let mark: yaml_mark_t = (*token).end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if (*token).type_ != YAML_KEY_TOKEN
            && (*token).type_ != YAML_VALUE_TOKEN
            && (*token).type_ != YAML_BLOCK_END_TOKEN
        {
            PUSH!(parser.states, YAML_PARSE_BLOCK_MAPPING_VALUE_STATE);
            yaml_parser_parse_node(parser, event, true, true)
        } else {
            parser.state = YAML_PARSE_BLOCK_MAPPING_VALUE_STATE;
            yaml_parser_process_empty_scalar(event, mark)
        }
    } else if (*token).type_ == YAML_BLOCK_END_TOKEN {
        parser.state = POP!(parser.states);
        let _ = POP!(parser.marks);
        *event = yaml_event_t::default();
        event.type_ = YAML_MAPPING_END_EVENT;
        event.start_mark = (*token).start_mark;
        event.end_mark = (*token).end_mark;
        SKIP_TOKEN(parser);
        Ok(())
    } else {
        let mark = POP!(parser.marks);
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
    if (*token).type_ == YAML_VALUE_TOKEN {
        let mark: yaml_mark_t = (*token).end_mark;
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if (*token).type_ != YAML_KEY_TOKEN
            && (*token).type_ != YAML_VALUE_TOKEN
            && (*token).type_ != YAML_BLOCK_END_TOKEN
        {
            PUSH!(parser.states, YAML_PARSE_BLOCK_MAPPING_KEY_STATE);
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
        PUSH!(parser.marks, (*token).start_mark);
        SKIP_TOKEN(parser);
    }
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if (*token).type_ != YAML_FLOW_SEQUENCE_END_TOKEN {
        if !first {
            if (*token).type_ == YAML_FLOW_ENTRY_TOKEN {
                SKIP_TOKEN(parser);
                token = PEEK_TOKEN(parser);
                if token.is_null() {
                    return Err(());
                }
            } else {
                let mark = POP!(parser.marks);
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
        if (*token).type_ == YAML_KEY_TOKEN {
            parser.state = YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_KEY_STATE;
            *event = yaml_event_t::default();
            event.type_ = YAML_MAPPING_START_EVENT;
            event.start_mark = (*token).start_mark;
            event.end_mark = (*token).end_mark;
            event.data.mapping_start.anchor = ptr::null_mut();
            event.data.mapping_start.tag = ptr::null_mut();
            event.data.mapping_start.implicit = true;
            event.data.mapping_start.style = YAML_FLOW_MAPPING_STYLE;
            SKIP_TOKEN(parser);
            return Ok(());
        } else if (*token).type_ != YAML_FLOW_SEQUENCE_END_TOKEN {
            PUSH!(parser.states, YAML_PARSE_FLOW_SEQUENCE_ENTRY_STATE);
            return yaml_parser_parse_node(parser, event, false, false);
        }
    }
    parser.state = POP!(parser.states);
    let _ = POP!(parser.marks);
    *event = yaml_event_t::default();
    event.type_ = YAML_SEQUENCE_END_EVENT;
    event.start_mark = (*token).start_mark;
    event.end_mark = (*token).end_mark;
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
    if (*token).type_ != YAML_VALUE_TOKEN
        && (*token).type_ != YAML_FLOW_ENTRY_TOKEN
        && (*token).type_ != YAML_FLOW_SEQUENCE_END_TOKEN
    {
        PUSH!(
            parser.states,
            YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_VALUE_STATE
        );
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
    if (*token).type_ == YAML_VALUE_TOKEN {
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if (*token).type_ != YAML_FLOW_ENTRY_TOKEN && (*token).type_ != YAML_FLOW_SEQUENCE_END_TOKEN
        {
            PUSH!(
                parser.states,
                YAML_PARSE_FLOW_SEQUENCE_ENTRY_MAPPING_END_STATE
            );
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
    *event = yaml_event_t::default();
    event.type_ = YAML_MAPPING_END_EVENT;
    event.start_mark = (*token).start_mark;
    event.end_mark = (*token).start_mark;
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
        PUSH!(parser.marks, (*token).start_mark);
        SKIP_TOKEN(parser);
    }
    token = PEEK_TOKEN(parser);
    if token.is_null() {
        return Err(());
    }
    if (*token).type_ != YAML_FLOW_MAPPING_END_TOKEN {
        if !first {
            if (*token).type_ == YAML_FLOW_ENTRY_TOKEN {
                SKIP_TOKEN(parser);
                token = PEEK_TOKEN(parser);
                if token.is_null() {
                    return Err(());
                }
            } else {
                let mark = POP!(parser.marks);
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
        if (*token).type_ == YAML_KEY_TOKEN {
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser);
            if token.is_null() {
                return Err(());
            }
            if (*token).type_ != YAML_VALUE_TOKEN
                && (*token).type_ != YAML_FLOW_ENTRY_TOKEN
                && (*token).type_ != YAML_FLOW_MAPPING_END_TOKEN
            {
                PUSH!(parser.states, YAML_PARSE_FLOW_MAPPING_VALUE_STATE);
                return yaml_parser_parse_node(parser, event, false, false);
            } else {
                parser.state = YAML_PARSE_FLOW_MAPPING_VALUE_STATE;
                return yaml_parser_process_empty_scalar(event, (*token).start_mark);
            }
        } else if (*token).type_ != YAML_FLOW_MAPPING_END_TOKEN {
            PUSH!(parser.states, YAML_PARSE_FLOW_MAPPING_EMPTY_VALUE_STATE);
            return yaml_parser_parse_node(parser, event, false, false);
        }
    }
    parser.state = POP!(parser.states);
    let _ = POP!(parser.marks);
    *event = yaml_event_t::default();
    event.type_ = YAML_MAPPING_END_EVENT;
    event.start_mark = (*token).start_mark;
    event.end_mark = (*token).end_mark;
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
    if (*token).type_ == YAML_VALUE_TOKEN {
        SKIP_TOKEN(parser);
        token = PEEK_TOKEN(parser);
        if token.is_null() {
            return Err(());
        }
        if (*token).type_ != YAML_FLOW_ENTRY_TOKEN && (*token).type_ != YAML_FLOW_MAPPING_END_TOKEN
        {
            PUSH!(parser.states, YAML_PARSE_FLOW_MAPPING_KEY_STATE);
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
    let value: *mut yaml_char_t = yaml_malloc(1_u64) as *mut yaml_char_t;
    *value = b'\0';
    *event = yaml_event_t::default();
    event.type_ = YAML_SCALAR_EVENT;
    event.start_mark = mark;
    event.end_mark = mark;
    event.data.scalar.anchor = ptr::null_mut::<yaml_char_t>();
    event.data.scalar.tag = ptr::null_mut::<yaml_char_t>();
    event.data.scalar.value = value;
    event.data.scalar.length = 0_u64;
    event.data.scalar.plain_implicit = true;
    event.data.scalar.quoted_implicit = false;
    event.data.scalar.style = YAML_PLAIN_SCALAR_STYLE;
    Ok(())
}

unsafe fn yaml_parser_process_directives(
    parser: &mut yaml_parser_t,
    version_directive_ref: *mut *mut yaml_version_directive_t,
    tag_directives_start_ref: *mut *mut yaml_tag_directive_t,
    tag_directives_end_ref: *mut *mut yaml_tag_directive_t,
) -> Result<(), ()> {
    let mut current_block: u64;
    let mut default_tag_directives: [yaml_tag_directive_t; 3] = [
        yaml_tag_directive_t {
            handle: b"!\0" as *const u8 as *const libc::c_char as *mut yaml_char_t,
            prefix: b"!\0" as *const u8 as *const libc::c_char as *mut yaml_char_t,
        },
        yaml_tag_directive_t {
            handle: b"!!\0" as *const u8 as *const libc::c_char as *mut yaml_char_t,
            prefix: b"tag:yaml.org,2002:\0" as *const u8 as *const libc::c_char as *mut yaml_char_t,
        },
        yaml_tag_directive_t {
            handle: ptr::null_mut::<yaml_char_t>(),
            prefix: ptr::null_mut::<yaml_char_t>(),
        },
    ];
    let mut default_tag_directive: *mut yaml_tag_directive_t;
    let mut version_directive: *mut yaml_version_directive_t =
        ptr::null_mut::<yaml_version_directive_t>();
    struct TagDirectives {
        start: *mut yaml_tag_directive_t,
        end: *mut yaml_tag_directive_t,
        top: *mut yaml_tag_directive_t,
    }
    let mut tag_directives = TagDirectives {
        start: ptr::null_mut::<yaml_tag_directive_t>(),
        end: ptr::null_mut::<yaml_tag_directive_t>(),
        top: ptr::null_mut::<yaml_tag_directive_t>(),
    };
    let mut token: *mut yaml_token_t;
    STACK_INIT!(tag_directives, yaml_tag_directive_t);
    token = PEEK_TOKEN(parser);
    if !token.is_null() {
        loop {
            if !((*token).type_ == YAML_VERSION_DIRECTIVE_TOKEN
                || (*token).type_ == YAML_TAG_DIRECTIVE_TOKEN)
            {
                current_block = 16924917904204750491;
                break;
            }
            if (*token).type_ == YAML_VERSION_DIRECTIVE_TOKEN {
                if !version_directive.is_null() {
                    yaml_parser_set_parser_error(
                        parser,
                        "found duplicate %YAML directive",
                        (*token).start_mark,
                    );
                    current_block = 17143798186130252483;
                    break;
                } else if (*token).data.version_directive.major != 1
                    || (*token).data.version_directive.minor != 1
                        && (*token).data.version_directive.minor != 2
                {
                    yaml_parser_set_parser_error(
                        parser,
                        "found incompatible YAML document",
                        (*token).start_mark,
                    );
                    current_block = 17143798186130252483;
                    break;
                } else {
                    version_directive =
                        yaml_malloc(size_of::<yaml_version_directive_t>() as libc::c_ulong)
                            as *mut yaml_version_directive_t;
                    (*version_directive).major = (*token).data.version_directive.major;
                    (*version_directive).minor = (*token).data.version_directive.minor;
                }
            } else if (*token).type_ == YAML_TAG_DIRECTIVE_TOKEN {
                let value = yaml_tag_directive_t {
                    handle: (*token).data.tag_directive.handle,
                    prefix: (*token).data.tag_directive.prefix,
                };
                if let Err(()) =
                    yaml_parser_append_tag_directive(parser, value, false, (*token).start_mark)
                {
                    current_block = 17143798186130252483;
                    break;
                }
                PUSH!(tag_directives, value);
            }
            SKIP_TOKEN(parser);
            token = PEEK_TOKEN(parser);
            if token.is_null() {
                current_block = 17143798186130252483;
                break;
            }
        }
        if current_block != 17143798186130252483 {
            default_tag_directive = default_tag_directives.as_mut_ptr();
            loop {
                if (*default_tag_directive).handle.is_null() {
                    current_block = 18377268871191777778;
                    break;
                }
                if let Err(()) = yaml_parser_append_tag_directive(
                    parser,
                    *default_tag_directive,
                    true,
                    (*token).start_mark,
                ) {
                    current_block = 17143798186130252483;
                    break;
                }
                default_tag_directive = default_tag_directive.wrapping_offset(1);
            }
            if current_block != 17143798186130252483 {
                if !version_directive_ref.is_null() {
                    *version_directive_ref = version_directive;
                }
                if !tag_directives_start_ref.is_null() {
                    if STACK_EMPTY!(tag_directives) {
                        *tag_directives_end_ref = ptr::null_mut::<yaml_tag_directive_t>();
                        *tag_directives_start_ref = *tag_directives_end_ref;
                        STACK_DEL!(tag_directives);
                    } else {
                        *tag_directives_start_ref = tag_directives.start;
                        *tag_directives_end_ref = tag_directives.top;
                    }
                } else {
                    STACK_DEL!(tag_directives);
                }
                if version_directive_ref.is_null() {
                    yaml_free(version_directive as *mut libc::c_void);
                }
                return Ok(());
            }
        }
    }
    yaml_free(version_directive as *mut libc::c_void);
    while !STACK_EMPTY!(tag_directives) {
        let tag_directive = POP!(tag_directives);
        yaml_free(tag_directive.handle as *mut libc::c_void);
        yaml_free(tag_directive.prefix as *mut libc::c_void);
    }
    STACK_DEL!(tag_directives);
    Err(())
}

unsafe fn yaml_parser_append_tag_directive(
    parser: &mut yaml_parser_t,
    value: yaml_tag_directive_t,
    allow_duplicates: bool,
    mark: yaml_mark_t,
) -> Result<(), ()> {
    let mut tag_directive: *mut yaml_tag_directive_t;
    let mut copy = yaml_tag_directive_t {
        handle: ptr::null_mut::<yaml_char_t>(),
        prefix: ptr::null_mut::<yaml_char_t>(),
    };
    tag_directive = parser.tag_directives.start;
    while tag_directive != parser.tag_directives.top {
        if strcmp(
            value.handle as *mut libc::c_char,
            (*tag_directive).handle as *mut libc::c_char,
        ) == 0
        {
            if allow_duplicates {
                return Ok(());
            }
            yaml_parser_set_parser_error(parser, "found duplicate %TAG directive", mark);
            return Err(());
        }
        tag_directive = tag_directive.wrapping_offset(1);
    }
    copy.handle = yaml_strdup(value.handle);
    copy.prefix = yaml_strdup(value.prefix);
    PUSH!(parser.tag_directives, copy);
    Ok(())
}
