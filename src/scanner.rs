use alloc::string::String;

use crate::macros::{is_blankz, is_break, vecdeque_starts_with};
use crate::ops::{ForceAdd as _, ForceMul as _};
use crate::reader::yaml_parser_update_buffer;
use crate::yaml::{ptrdiff_t, size_t, YamlTokenData};
use crate::{
    libc, yaml_mark_t, yaml_parser_t, yaml_simple_key_t, yaml_token_t, ReaderError, ScannerError,
    YAML_DOUBLE_QUOTED_SCALAR_STYLE, YAML_FOLDED_SCALAR_STYLE, YAML_LITERAL_SCALAR_STYLE,
    YAML_PLAIN_SCALAR_STYLE, YAML_SINGLE_QUOTED_SCALAR_STYLE,
};

fn CACHE(parser: &mut yaml_parser_t, length: size_t) -> Result<(), ReaderError> {
    if parser.unread >= length {
        Ok(())
    } else {
        yaml_parser_update_buffer(parser, length)
    }
}

fn SKIP(parser: &mut yaml_parser_t) {
    let popped = parser.buffer.pop_front().expect("unexpected end of tokens");
    let width = popped.len_utf8();
    parser.mark.index += width as u64;
    parser.mark.column += 1;
    parser.unread -= 1;
}

fn SKIP_LINE(parser: &mut yaml_parser_t) {
    if vecdeque_starts_with(&parser.buffer, &['\r', '\n']) {
        parser.mark.index += 2;
        parser.mark.column = 0;
        parser.mark.line += 1;
        parser.unread -= 2;
        parser.buffer.drain(0..2);
    } else if let Some(front) = parser.buffer.front().copied() {
        if is_break(front) {
            let width = front.len_utf8();
            parser.mark.index += width as u64;
            parser.mark.column = 0;
            parser.mark.line += 1;
            parser.unread -= 1;
            parser.buffer.pop_front();
        }
    }
}

fn READ_STRING(parser: &mut yaml_parser_t, string: &mut String) {
    if let Some(popped) = parser.buffer.pop_front() {
        string.push(popped);
        parser.mark.index = popped.len_utf8() as u64;
        parser.mark.column += 1;
        parser.unread -= 1;
    } else {
        panic!("unexpected end of input")
    }
}

fn READ_LINE_STRING(parser: &mut yaml_parser_t, string: &mut String) {
    if vecdeque_starts_with(&parser.buffer, &['\r', '\n']) {
        string.push('\n');
        parser.buffer.drain(0..2);
        parser.mark.index += 2;
        parser.mark.column = 0;
        parser.mark.line += 1;
        parser.unread -= 2;
    } else {
        let Some(front) = parser.buffer.front().copied() else {
            panic!("unexpected end of input");
        };
        if is_break(front) {
            parser.buffer.pop_front();
            let char_len = front.len_utf8();
            if char_len == 3 {
                // libyaml preserves Unicode breaks in this case.
                string.push(front);
            } else {
                string.push('\n');
            }
            parser.mark.index += char_len as u64;
            parser.mark.column = 0;
            parser.mark.line += 1;
            parser.unread -= 1;
        }
    }
}

/// Scan the input stream and produce the next token.
///
/// Call the function subsequently to produce a sequence of tokens corresponding
/// to the input stream. The initial token has the type YAML_STREAM_START_TOKEN
/// while the ending token has the type YAML_STREAM_END_TOKEN.
///
/// An application is responsible for freeing any buffers associated with the
/// produced token object using the yaml_token_delete function.
///
/// An application must not alternate the calls of yaml_parser_scan() with the
/// calls of yaml_parser_parse() or yaml_parser_load(). Doing this will break
/// the parser.
pub fn yaml_parser_scan(parser: &mut yaml_parser_t) -> Result<yaml_token_t, ScannerError> {
    if parser.stream_end_produced {
        return Ok(yaml_token_t {
            data: YamlTokenData::StreamEnd,
            ..Default::default()
        });
    }
    if !parser.token_available {
        yaml_parser_fetch_more_tokens(parser)?;
    }
    if let Some(token) = parser.tokens.pop_front() {
        parser.token_available = false;
        parser.tokens_parsed = parser.tokens_parsed.force_add(1);
        if let YamlTokenData::StreamEnd = &token.data {
            parser.stream_end_produced = true;
        }
        Ok(token)
    } else {
        unreachable!("no more tokens, but stream-end was not produced")
    }
}

fn yaml_parser_set_scanner_error<T>(
    parser: &mut yaml_parser_t,
    context: &'static str,
    context_mark: yaml_mark_t,
    problem: &'static str,
) -> Result<T, ScannerError> {
    Err(ScannerError::Problem {
        context,
        context_mark,
        problem,
        problem_mark: parser.mark,
    })
}

pub(crate) fn yaml_parser_fetch_more_tokens(
    parser: &mut yaml_parser_t,
) -> Result<(), ScannerError> {
    let mut need_more_tokens;
    loop {
        need_more_tokens = false;
        if parser.tokens.is_empty() {
            need_more_tokens = true;
        } else {
            yaml_parser_stale_simple_keys(parser)?;
            for simple_key in &parser.simple_keys {
                if simple_key.possible && simple_key.token_number == parser.tokens_parsed {
                    need_more_tokens = true;
                    break;
                }
            }
        }
        if !need_more_tokens {
            break;
        }
        yaml_parser_fetch_next_token(parser)?;
    }
    parser.token_available = true;
    Ok(())
}

fn yaml_parser_fetch_next_token(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    CACHE(parser, 1_u64)?;
    if !parser.stream_start_produced {
        yaml_parser_fetch_stream_start(parser);
        return Ok(());
    }
    yaml_parser_scan_to_next_token(parser)?;
    yaml_parser_stale_simple_keys(parser)?;
    yaml_parser_unroll_indent(parser, parser.mark.column as ptrdiff_t);
    CACHE(parser, 4_u64)?;
    if IS_Z!(parser.buffer) {
        return yaml_parser_fetch_stream_end(parser);
    }
    if parser.mark.column == 0_u64 && parser.buffer[0] == '%' {
        return yaml_parser_fetch_directive(parser);
    }
    if parser.mark.column == 0_u64
        && CHECK_AT!(parser.buffer, '-', 0)
        && CHECK_AT!(parser.buffer, '-', 1)
        && CHECK_AT!(parser.buffer, '-', 2)
        && is_blankz(parser.buffer.get(3).copied())
    {
        return yaml_parser_fetch_document_indicator(parser, YamlTokenData::DocumentStart);
    }
    if parser.mark.column == 0_u64
        && CHECK_AT!(parser.buffer, '.', 0)
        && CHECK_AT!(parser.buffer, '.', 1)
        && CHECK_AT!(parser.buffer, '.', 2)
        && is_blankz(parser.buffer.get(3).copied())
    {
        return yaml_parser_fetch_document_indicator(parser, YamlTokenData::DocumentEnd);
    }
    if CHECK!(parser.buffer, '[') {
        return yaml_parser_fetch_flow_collection_start(parser, YamlTokenData::FlowSequenceStart);
    }
    if CHECK!(parser.buffer, '{') {
        return yaml_parser_fetch_flow_collection_start(parser, YamlTokenData::FlowMappingStart);
    }
    if CHECK!(parser.buffer, ']') {
        return yaml_parser_fetch_flow_collection_end(parser, YamlTokenData::FlowSequenceEnd);
    }
    if CHECK!(parser.buffer, '}') {
        return yaml_parser_fetch_flow_collection_end(parser, YamlTokenData::FlowMappingEnd);
    }
    if CHECK!(parser.buffer, ',') {
        return yaml_parser_fetch_flow_entry(parser);
    }
    if CHECK!(parser.buffer, '-') && IS_BLANKZ_AT!(parser.buffer, 1) {
        return yaml_parser_fetch_block_entry(parser);
    }
    if CHECK!(parser.buffer, '?') && (parser.flow_level != 0 || IS_BLANKZ_AT!(parser.buffer, 1)) {
        return yaml_parser_fetch_key(parser);
    }
    if CHECK!(parser.buffer, ':') && (parser.flow_level != 0 || IS_BLANKZ_AT!(parser.buffer, 1)) {
        return yaml_parser_fetch_value(parser);
    }
    if CHECK!(parser.buffer, '*') {
        return yaml_parser_fetch_anchor(parser, true);
    }
    if CHECK!(parser.buffer, '&') {
        return yaml_parser_fetch_anchor(parser, false);
    }
    if CHECK!(parser.buffer, '!') {
        return yaml_parser_fetch_tag(parser);
    }
    if CHECK!(parser.buffer, '|') && parser.flow_level == 0 {
        return yaml_parser_fetch_block_scalar(parser, true);
    }
    if CHECK!(parser.buffer, '>') && parser.flow_level == 0 {
        return yaml_parser_fetch_block_scalar(parser, false);
    }
    if CHECK!(parser.buffer, '\'') {
        return yaml_parser_fetch_flow_scalar(parser, true);
    }
    if CHECK!(parser.buffer, '"') {
        return yaml_parser_fetch_flow_scalar(parser, false);
    }
    if !(IS_BLANKZ!(parser.buffer)
        || CHECK!(parser.buffer, '-')
        || CHECK!(parser.buffer, '?')
        || CHECK!(parser.buffer, ':')
        || CHECK!(parser.buffer, ',')
        || CHECK!(parser.buffer, '[')
        || CHECK!(parser.buffer, ']')
        || CHECK!(parser.buffer, '{')
        || CHECK!(parser.buffer, '}')
        || CHECK!(parser.buffer, '#')
        || CHECK!(parser.buffer, '&')
        || CHECK!(parser.buffer, '*')
        || CHECK!(parser.buffer, '!')
        || CHECK!(parser.buffer, '|')
        || CHECK!(parser.buffer, '>')
        || CHECK!(parser.buffer, '\'')
        || CHECK!(parser.buffer, '"')
        || CHECK!(parser.buffer, '%')
        || CHECK!(parser.buffer, '@')
        || CHECK!(parser.buffer, '`'))
        || CHECK!(parser.buffer, '-') && !IS_BLANK_AT!(parser.buffer, 1)
        || parser.flow_level == 0
            && (CHECK!(parser.buffer, '?') || CHECK!(parser.buffer, ':'))
            && !IS_BLANKZ_AT!(parser.buffer, 1)
    {
        return yaml_parser_fetch_plain_scalar(parser);
    }
    yaml_parser_set_scanner_error(
        parser,
        "while scanning for the next token",
        parser.mark,
        "found character that cannot start any token",
    )
}

fn yaml_parser_stale_simple_keys(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    for simple_key in &mut parser.simple_keys {
        let mark = simple_key.mark;
        if simple_key.possible
            && (mark.line < parser.mark.line || mark.index.force_add(1024_u64) < parser.mark.index)
        {
            if simple_key.required {
                return yaml_parser_set_scanner_error(
                    parser,
                    "while scanning a simple key",
                    mark,
                    "could not find expected ':'",
                );
            }
            simple_key.possible = false;
        }
    }

    Ok(())
}

fn yaml_parser_save_simple_key(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    let required =
        parser.flow_level == 0 && parser.indent as libc::c_long == parser.mark.column as ptrdiff_t;
    if parser.simple_key_allowed {
        let simple_key = yaml_simple_key_t {
            possible: true,
            required,
            token_number: parser
                .tokens_parsed
                .force_add(parser.tokens.len() as libc::c_ulong),
            mark: parser.mark,
        };
        yaml_parser_remove_simple_key(parser)?;
        *parser.simple_keys.last_mut().unwrap() = simple_key;
    }
    Ok(())
}

fn yaml_parser_remove_simple_key(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    let simple_key: &mut yaml_simple_key_t = parser.simple_keys.last_mut().unwrap();
    if simple_key.possible {
        let mark = simple_key.mark;
        if simple_key.required {
            return yaml_parser_set_scanner_error(
                parser,
                "while scanning a simple key",
                mark,
                "could not find expected ':'",
            );
        }
    }
    simple_key.possible = false;
    Ok(())
}

fn yaml_parser_increase_flow_level(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    let empty_simple_key = yaml_simple_key_t {
        possible: false,
        required: false,
        token_number: 0_u64,
        mark: yaml_mark_t {
            index: 0_u64,
            line: 0_u64,
            column: 0_u64,
        },
    };
    parser.simple_keys.push(empty_simple_key);
    assert!(
        !(parser.flow_level == libc::c_int::MAX),
        "parser.flow_level integer overflow"
    );
    parser.flow_level += 1;
    Ok(())
}

fn yaml_parser_decrease_flow_level(parser: &mut yaml_parser_t) {
    if parser.flow_level != 0 {
        parser.flow_level -= 1;
        let _ = parser.simple_keys.pop();
    }
}

fn yaml_parser_roll_indent(
    parser: &mut yaml_parser_t,
    column: ptrdiff_t,
    number: ptrdiff_t,
    data: YamlTokenData,
    mark: yaml_mark_t,
) -> Result<(), ScannerError> {
    if parser.flow_level != 0 {
        return Ok(());
    }
    if (parser.indent as libc::c_long) < column {
        parser.indents.push(parser.indent);
        assert!(
            !(column > ptrdiff_t::from(libc::c_int::MAX)),
            "integer overflow"
        );
        parser.indent = column as libc::c_int;
        let token = yaml_token_t {
            data,
            start_mark: mark,
            end_mark: mark,
        };
        if number == -1_i64 {
            parser.tokens.push_back(token);
        } else {
            parser.tokens.insert(
                (number as libc::c_ulong).wrapping_sub(parser.tokens_parsed) as usize,
                token,
            );
        }
    }
    Ok(())
}

fn yaml_parser_unroll_indent(parser: &mut yaml_parser_t, column: ptrdiff_t) {
    if parser.flow_level != 0 {
        return;
    }
    while parser.indent as libc::c_long > column {
        let token = yaml_token_t {
            data: YamlTokenData::BlockEnd,
            start_mark: parser.mark,
            end_mark: parser.mark,
        };
        parser.tokens.push_back(token);
        parser.indent = parser.indents.pop().unwrap();
    }
}

fn yaml_parser_fetch_stream_start(parser: &mut yaml_parser_t) {
    let simple_key = yaml_simple_key_t {
        possible: false,
        required: false,
        token_number: 0_u64,
        mark: yaml_mark_t {
            index: 0_u64,
            line: 0_u64,
            column: 0_u64,
        },
    };
    parser.indent = -1;
    parser.simple_keys.push(simple_key);
    parser.simple_key_allowed = true;
    parser.stream_start_produced = true;
    let token = yaml_token_t {
        data: YamlTokenData::StreamStart {
            encoding: parser.encoding,
        },
        start_mark: parser.mark,
        end_mark: parser.mark,
    };
    parser.tokens.push_back(token);
}

fn yaml_parser_fetch_stream_end(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    if parser.mark.column != 0_u64 {
        parser.mark.column = 0_u64;
        parser.mark.line = parser.mark.line.force_add(1);
    }
    yaml_parser_unroll_indent(parser, -1_i64);
    yaml_parser_remove_simple_key(parser)?;
    parser.simple_key_allowed = false;
    let token = yaml_token_t {
        data: YamlTokenData::StreamEnd,
        start_mark: parser.mark,
        end_mark: parser.mark,
    };
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_directive(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    let mut token = yaml_token_t::default();
    yaml_parser_unroll_indent(parser, -1_i64);
    yaml_parser_remove_simple_key(parser)?;
    parser.simple_key_allowed = false;
    yaml_parser_scan_directive(parser, &mut token)?;
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_document_indicator(
    parser: &mut yaml_parser_t,
    data: YamlTokenData,
) -> Result<(), ScannerError> {
    yaml_parser_unroll_indent(parser, -1_i64);
    yaml_parser_remove_simple_key(parser)?;
    parser.simple_key_allowed = false;
    let start_mark: yaml_mark_t = parser.mark;
    SKIP(parser);
    SKIP(parser);
    SKIP(parser);
    let end_mark: yaml_mark_t = parser.mark;

    let token = yaml_token_t {
        data,
        start_mark,
        end_mark,
    };
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_flow_collection_start(
    parser: &mut yaml_parser_t,
    data: YamlTokenData,
) -> Result<(), ScannerError> {
    yaml_parser_save_simple_key(parser)?;
    yaml_parser_increase_flow_level(parser)?;
    parser.simple_key_allowed = true;
    let start_mark: yaml_mark_t = parser.mark;
    SKIP(parser);
    let end_mark: yaml_mark_t = parser.mark;
    let token = yaml_token_t {
        data,
        start_mark,
        end_mark,
    };
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_flow_collection_end(
    parser: &mut yaml_parser_t,
    data: YamlTokenData,
) -> Result<(), ScannerError> {
    yaml_parser_remove_simple_key(parser)?;
    yaml_parser_decrease_flow_level(parser);
    parser.simple_key_allowed = false;
    let start_mark: yaml_mark_t = parser.mark;
    SKIP(parser);
    let end_mark: yaml_mark_t = parser.mark;
    let token = yaml_token_t {
        data,
        start_mark,
        end_mark,
    };
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_flow_entry(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    yaml_parser_remove_simple_key(parser)?;
    parser.simple_key_allowed = true;
    let start_mark: yaml_mark_t = parser.mark;
    SKIP(parser);
    let end_mark: yaml_mark_t = parser.mark;
    let token = yaml_token_t {
        data: YamlTokenData::FlowEntry,
        start_mark,
        end_mark,
    };
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_block_entry(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    if parser.flow_level == 0 {
        if !parser.simple_key_allowed {
            return yaml_parser_set_scanner_error(
                parser,
                "",
                parser.mark,
                "block sequence entries are not allowed in this context",
            );
        }
        yaml_parser_roll_indent(
            parser,
            parser.mark.column as ptrdiff_t,
            -1_i64,
            YamlTokenData::BlockSequenceStart,
            parser.mark,
        )?;
    }
    yaml_parser_remove_simple_key(parser)?;
    parser.simple_key_allowed = true;
    let start_mark: yaml_mark_t = parser.mark;
    SKIP(parser);
    let end_mark: yaml_mark_t = parser.mark;
    let token = yaml_token_t {
        data: YamlTokenData::BlockEntry,
        start_mark,
        end_mark,
    };
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_key(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    if parser.flow_level == 0 {
        if !parser.simple_key_allowed {
            return yaml_parser_set_scanner_error(
                parser,
                "",
                parser.mark,
                "mapping keys are not allowed in this context",
            );
        }
        yaml_parser_roll_indent(
            parser,
            parser.mark.column as ptrdiff_t,
            -1_i64,
            YamlTokenData::BlockMappingStart,
            parser.mark,
        )?;
    }
    yaml_parser_remove_simple_key(parser)?;
    parser.simple_key_allowed = parser.flow_level == 0;
    let start_mark: yaml_mark_t = parser.mark;
    SKIP(parser);
    let end_mark: yaml_mark_t = parser.mark;
    let token = yaml_token_t {
        data: YamlTokenData::Key,
        start_mark,
        end_mark,
    };
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_value(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    let simple_key: &mut yaml_simple_key_t = parser.simple_keys.last_mut().unwrap();
    if simple_key.possible {
        let token = yaml_token_t {
            data: YamlTokenData::Key,
            start_mark: simple_key.mark,
            end_mark: simple_key.mark,
        };
        parser.tokens.insert(
            simple_key.token_number.wrapping_sub(parser.tokens_parsed) as usize,
            token,
        );
        let mark_column = simple_key.mark.column as ptrdiff_t;
        let token_number = simple_key.token_number as ptrdiff_t;
        let mark = simple_key.mark;
        simple_key.possible = false;
        yaml_parser_roll_indent(
            parser,
            mark_column,
            token_number,
            YamlTokenData::BlockMappingStart,
            mark,
        )?;
        parser.simple_key_allowed = false;
    } else {
        if parser.flow_level == 0 {
            if !parser.simple_key_allowed {
                return yaml_parser_set_scanner_error(
                    parser,
                    "",
                    parser.mark,
                    "mapping values are not allowed in this context",
                );
            }
            yaml_parser_roll_indent(
                parser,
                parser.mark.column as ptrdiff_t,
                -1_i64,
                YamlTokenData::BlockMappingStart,
                parser.mark,
            )?;
        }
        parser.simple_key_allowed = parser.flow_level == 0;
    }
    let start_mark: yaml_mark_t = parser.mark;
    SKIP(parser);
    let end_mark: yaml_mark_t = parser.mark;
    let token = yaml_token_t {
        data: YamlTokenData::Value,
        start_mark,
        end_mark,
    };
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_anchor(
    parser: &mut yaml_parser_t,
    fetch_alias_instead_of_anchor: bool,
) -> Result<(), ScannerError> {
    let mut token = yaml_token_t::default();
    yaml_parser_save_simple_key(parser)?;
    parser.simple_key_allowed = false;
    yaml_parser_scan_anchor(parser, &mut token, fetch_alias_instead_of_anchor)?;
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_tag(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    let mut token = yaml_token_t::default();
    yaml_parser_save_simple_key(parser)?;
    parser.simple_key_allowed = false;
    yaml_parser_scan_tag(parser, &mut token)?;
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_block_scalar(
    parser: &mut yaml_parser_t,
    literal: bool,
) -> Result<(), ScannerError> {
    let mut token = yaml_token_t::default();
    yaml_parser_remove_simple_key(parser)?;
    parser.simple_key_allowed = true;
    yaml_parser_scan_block_scalar(parser, &mut token, literal)?;
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_flow_scalar(
    parser: &mut yaml_parser_t,
    single: bool,
) -> Result<(), ScannerError> {
    let mut token = yaml_token_t::default();
    yaml_parser_save_simple_key(parser)?;
    parser.simple_key_allowed = false;
    yaml_parser_scan_flow_scalar(parser, &mut token, single)?;
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_fetch_plain_scalar(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    let mut token = yaml_token_t::default();
    yaml_parser_save_simple_key(parser)?;
    parser.simple_key_allowed = false;
    yaml_parser_scan_plain_scalar(parser, &mut token)?;
    parser.tokens.push_back(token);
    Ok(())
}

fn yaml_parser_scan_to_next_token(parser: &mut yaml_parser_t) -> Result<(), ScannerError> {
    loop {
        CACHE(parser, 1_u64)?;
        if parser.mark.column == 0_u64 && IS_BOM!(parser.buffer) {
            SKIP(parser);
        }
        CACHE(parser, 1_u64)?;
        while CHECK!(parser.buffer, ' ')
            || (parser.flow_level != 0 || !parser.simple_key_allowed) && CHECK!(parser.buffer, '\t')
        {
            SKIP(parser);
            CACHE(parser, 1_u64)?;
        }
        if CHECK!(parser.buffer, '#') {
            while !IS_BREAKZ!(parser.buffer) {
                SKIP(parser);
                CACHE(parser, 1_u64)?;
            }
        }
        if !IS_BREAK!(parser.buffer) {
            break;
        }
        CACHE(parser, 2_u64)?;
        SKIP_LINE(parser);
        if parser.flow_level == 0 {
            parser.simple_key_allowed = true;
        }
    }
    Ok(())
}

fn yaml_parser_scan_directive(
    parser: &mut yaml_parser_t,
    token: &mut yaml_token_t,
) -> Result<(), ScannerError> {
    let end_mark: yaml_mark_t;
    let mut major: libc::c_int = 0;
    let mut minor: libc::c_int = 0;
    let start_mark: yaml_mark_t = parser.mark;
    SKIP(parser);
    let name = yaml_parser_scan_directive_name(parser, start_mark)?;
    if name == "YAML" {
        yaml_parser_scan_version_directive_value(parser, start_mark, &mut major, &mut minor)?;

        end_mark = parser.mark;
        *token = yaml_token_t {
            data: YamlTokenData::VersionDirective { major, minor },
            start_mark,
            end_mark,
        };
    } else if name == "TAG" {
        let (handle, prefix) = yaml_parser_scan_tag_directive_value(parser, start_mark)?;
        end_mark = parser.mark;
        *token = yaml_token_t {
            data: YamlTokenData::TagDirective { handle, prefix },
            start_mark,
            end_mark,
        };
    } else {
        return yaml_parser_set_scanner_error(
            parser,
            "while scanning a directive",
            start_mark,
            "found unknown directive name",
        );
    }
    CACHE(parser, 1_u64)?;
    loop {
        if !IS_BLANK!(parser.buffer) {
            break;
        }
        SKIP(parser);
        CACHE(parser, 1_u64)?;
    }

    if CHECK!(parser.buffer, '#') {
        loop {
            if IS_BREAKZ!(parser.buffer) {
                break;
            }
            SKIP(parser);
            CACHE(parser, 1_u64)?;
        }
    }

    if !IS_BREAKZ!(parser.buffer) {
        yaml_parser_set_scanner_error(
            parser,
            "while scanning a directive",
            start_mark,
            "did not find expected comment or line break",
        )
    } else {
        if IS_BREAK!(parser.buffer) {
            CACHE(parser, 2_u64)?;
            SKIP_LINE(parser);
        }
        Ok(())
    }
}

fn yaml_parser_scan_directive_name(
    parser: &mut yaml_parser_t,
    start_mark: yaml_mark_t,
) -> Result<String, ScannerError> {
    let mut string = String::new();
    CACHE(parser, 1_u64)?;

    loop {
        if !IS_ALPHA!(parser.buffer) {
            break;
        }
        READ_STRING(parser, &mut string);
        CACHE(parser, 1_u64)?;
    }

    if string.is_empty() {
        yaml_parser_set_scanner_error(
            parser,
            "while scanning a directive",
            start_mark,
            "could not find expected directive name",
        )
    } else if !IS_BLANKZ!(parser.buffer) {
        yaml_parser_set_scanner_error(
            parser,
            "while scanning a directive",
            start_mark,
            "found unexpected non-alphabetical character",
        )
    } else {
        Ok(string)
    }
}

fn yaml_parser_scan_version_directive_value(
    parser: &mut yaml_parser_t,
    start_mark: yaml_mark_t,
    major: &mut libc::c_int,
    minor: &mut libc::c_int,
) -> Result<(), ScannerError> {
    CACHE(parser, 1_u64)?;
    while IS_BLANK!(parser.buffer) {
        SKIP(parser);
        CACHE(parser, 1_u64)?;
    }
    yaml_parser_scan_version_directive_number(parser, start_mark, major)?;
    if !CHECK!(parser.buffer, '.') {
        return yaml_parser_set_scanner_error(
            parser,
            "while scanning a %YAML directive",
            start_mark,
            "did not find expected digit or '.' character",
        );
    }
    SKIP(parser);
    yaml_parser_scan_version_directive_number(parser, start_mark, minor)
}

const MAX_NUMBER_LENGTH: u64 = 9_u64;

fn yaml_parser_scan_version_directive_number(
    parser: &mut yaml_parser_t,
    start_mark: yaml_mark_t,
    number: &mut libc::c_int,
) -> Result<(), ScannerError> {
    let mut value: libc::c_int = 0;
    let mut length: size_t = 0_u64;
    CACHE(parser, 1_u64)?;
    while IS_DIGIT!(parser.buffer) {
        length = length.force_add(1);
        if length > MAX_NUMBER_LENGTH {
            return yaml_parser_set_scanner_error(
                parser,
                "while scanning a %YAML directive",
                start_mark,
                "found extremely long version number",
            );
        }
        value = value
            .force_mul(10)
            .force_add(AS_DIGIT!(parser.buffer) as i32);
        SKIP(parser);
        CACHE(parser, 1_u64)?;
    }
    if length == 0 {
        return yaml_parser_set_scanner_error(
            parser,
            "while scanning a %YAML directive",
            start_mark,
            "did not find expected version number",
        );
    }
    *number = value;
    Ok(())
}

// Returns (handle, prefix)
fn yaml_parser_scan_tag_directive_value(
    parser: &mut yaml_parser_t,
    start_mark: yaml_mark_t,
) -> Result<(String, String), ScannerError> {
    CACHE(parser, 1_u64)?;

    loop {
        if IS_BLANK!(parser.buffer) {
            SKIP(parser);
            CACHE(parser, 1_u64)?;
        } else {
            let handle_value = yaml_parser_scan_tag_handle(parser, true, start_mark)?;

            CACHE(parser, 1_u64)?;

            if !IS_BLANK!(parser.buffer) {
                return yaml_parser_set_scanner_error(
                    parser,
                    "while scanning a %TAG directive",
                    start_mark,
                    "did not find expected whitespace",
                );
            } else {
                while IS_BLANK!(parser.buffer) {
                    SKIP(parser);
                    CACHE(parser, 1_u64)?;
                }

                let prefix_value = yaml_parser_scan_tag_uri(parser, true, true, None, start_mark)?;
                CACHE(parser, 1_u64)?;

                if !IS_BLANKZ!(parser.buffer) {
                    return yaml_parser_set_scanner_error(
                        parser,
                        "while scanning a %TAG directive",
                        start_mark,
                        "did not find expected whitespace or line break",
                    );
                } else {
                    return Ok((handle_value, prefix_value));
                }
            }
        }
    }
}

fn yaml_parser_scan_anchor(
    parser: &mut yaml_parser_t,
    token: &mut yaml_token_t,
    scan_alias_instead_of_anchor: bool,
) -> Result<(), ScannerError> {
    let mut length: libc::c_int = 0;

    let mut string = String::new();
    let start_mark: yaml_mark_t = parser.mark;
    SKIP(parser);
    CACHE(parser, 1_u64)?;

    loop {
        if !IS_ALPHA!(parser.buffer) {
            break;
        }
        READ_STRING(parser, &mut string);
        CACHE(parser, 1_u64)?;
        length += 1;
    }
    let end_mark: yaml_mark_t = parser.mark;
    if length == 0
        || !(IS_BLANKZ!(parser.buffer)
            || CHECK!(parser.buffer, '?')
            || CHECK!(parser.buffer, ':')
            || CHECK!(parser.buffer, ',')
            || CHECK!(parser.buffer, ']')
            || CHECK!(parser.buffer, '}')
            || CHECK!(parser.buffer, '%')
            || CHECK!(parser.buffer, '@')
            || CHECK!(parser.buffer, '`'))
    {
        yaml_parser_set_scanner_error(
            parser,
            if !scan_alias_instead_of_anchor {
                "while scanning an anchor"
            } else {
                "while scanning an alias"
            },
            start_mark,
            "did not find expected alphabetic or numeric character",
        )
    } else {
        *token = yaml_token_t {
            data: if scan_alias_instead_of_anchor {
                YamlTokenData::Alias { value: string }
            } else {
                YamlTokenData::Anchor { value: string }
            },
            start_mark,
            end_mark,
        };
        Ok(())
    }
}

fn yaml_parser_scan_tag(
    parser: &mut yaml_parser_t,
    token: &mut yaml_token_t,
) -> Result<(), ScannerError> {
    let mut handle;
    let mut suffix;

    let start_mark: yaml_mark_t = parser.mark;

    CACHE(parser, 2_u64)?;

    if CHECK_AT!(parser.buffer, '<', 1) {
        handle = String::new();
        SKIP(parser);
        SKIP(parser);
        suffix = yaml_parser_scan_tag_uri(parser, true, false, None, start_mark)?;

        if !CHECK!(parser.buffer, '>') {
            return yaml_parser_set_scanner_error(
                parser,
                "while scanning a tag",
                start_mark,
                "did not find the expected '>'",
            );
        } else {
            SKIP(parser);
        }
    } else {
        handle = yaml_parser_scan_tag_handle(parser, false, start_mark)?;
        if handle.starts_with('!') && handle.len() > 1 && handle.ends_with('!') {
            suffix = yaml_parser_scan_tag_uri(parser, false, false, None, start_mark)?;
        } else {
            suffix = yaml_parser_scan_tag_uri(parser, false, false, Some(&handle), start_mark)?;
            handle = String::from("!");
            if suffix.is_empty() {
                core::mem::swap(&mut handle, &mut suffix);
            }
        }
    }

    CACHE(parser, 1_u64)?;
    if !IS_BLANKZ!(parser.buffer) {
        if parser.flow_level == 0 || !CHECK!(parser.buffer, ',') {
            return yaml_parser_set_scanner_error(
                parser,
                "while scanning a tag",
                start_mark,
                "did not find expected whitespace or line break",
            );
        } else {
            panic!("TODO: What is expected here?");
        }
    }

    let end_mark: yaml_mark_t = parser.mark;
    *token = yaml_token_t {
        data: YamlTokenData::Tag { handle, suffix },
        start_mark,
        end_mark,
    };

    Ok(())
}

fn yaml_parser_scan_tag_handle(
    parser: &mut yaml_parser_t,
    directive: bool,
    start_mark: yaml_mark_t,
) -> Result<String, ScannerError> {
    let mut string = String::new();
    CACHE(parser, 1_u64)?;

    if !CHECK!(parser.buffer, '!') {
        return yaml_parser_set_scanner_error(
            parser,
            if directive {
                "while scanning a tag directive"
            } else {
                "while scanning a tag"
            },
            start_mark,
            "did not find expected '!'",
        );
    }

    READ_STRING(parser, &mut string);
    CACHE(parser, 1_u64)?;
    loop {
        if !IS_ALPHA!(parser.buffer) {
            break;
        }
        READ_STRING(parser, &mut string);
        CACHE(parser, 1_u64)?;
    }
    if CHECK!(parser.buffer, '!') {
        READ_STRING(parser, &mut string);
    } else if directive && string != "!" {
        return yaml_parser_set_scanner_error(
            parser,
            "while parsing a tag directive",
            start_mark,
            "did not find expected '!'",
        );
    }
    Ok(string)
}

fn yaml_parser_scan_tag_uri(
    parser: &mut yaml_parser_t,
    uri_char: bool,
    directive: bool,
    head: Option<&str>,
    start_mark: yaml_mark_t,
) -> Result<String, ScannerError> {
    let head = head.unwrap_or("");
    let mut length = head.len();
    let mut string = String::new();

    if length > 1 {
        string = String::from(&head[1..]);
    }
    CACHE(parser, 1_u64)?;

    while IS_ALPHA!(parser.buffer)
        || CHECK!(parser.buffer, ';')
        || CHECK!(parser.buffer, '/')
        || CHECK!(parser.buffer, '?')
        || CHECK!(parser.buffer, ':')
        || CHECK!(parser.buffer, '@')
        || CHECK!(parser.buffer, '&')
        || CHECK!(parser.buffer, '=')
        || CHECK!(parser.buffer, '+')
        || CHECK!(parser.buffer, '$')
        || CHECK!(parser.buffer, '.')
        || CHECK!(parser.buffer, '%')
        || CHECK!(parser.buffer, '!')
        || CHECK!(parser.buffer, '~')
        || CHECK!(parser.buffer, '*')
        || CHECK!(parser.buffer, '\'')
        || CHECK!(parser.buffer, '(')
        || CHECK!(parser.buffer, ')')
        || uri_char
            && (CHECK!(parser.buffer, ',')
                || CHECK!(parser.buffer, '[')
                || CHECK!(parser.buffer, ']'))
    {
        if CHECK!(parser.buffer, '%') {
            yaml_parser_scan_uri_escapes(parser, directive, start_mark, &mut string)?;
        } else {
            READ_STRING(parser, &mut string);
        }
        length = length.force_add(1);
        CACHE(parser, 1_u64)?;
    }
    if length == 0 {
        yaml_parser_set_scanner_error(
            parser,
            if directive {
                "while parsing a %TAG directive"
            } else {
                "while parsing a tag"
            },
            start_mark,
            "did not find expected tag URI",
        )
    } else {
        Ok(string)
    }
}

fn yaml_parser_scan_uri_escapes(
    parser: &mut yaml_parser_t,
    directive: bool,
    start_mark: yaml_mark_t,
    string: &mut String,
) -> Result<(), ScannerError> {
    let mut width: libc::c_int = 0;
    loop {
        CACHE(parser, 3_u64)?;
        if !(CHECK!(parser.buffer, '%')
            && IS_HEX_AT!(parser.buffer, 1)
            && IS_HEX_AT!(parser.buffer, 2))
        {
            return yaml_parser_set_scanner_error(
                parser,
                if directive {
                    "while parsing a %TAG directive"
                } else {
                    "while parsing a tag"
                },
                start_mark,
                "did not find URI escaped octet",
            );
        }
        let octet: libc::c_uchar =
            ((AS_HEX_AT!(parser.buffer, 1) << 4) + AS_HEX_AT!(parser.buffer, 2)) as libc::c_uchar;
        if width == 0 {
            width = if octet & 0x80 == 0 {
                1
            } else if octet & 0xE0 == 0xC0 {
                2
            } else if octet & 0xF0 == 0xE0 {
                3
            } else if octet & 0xF8 == 0xF0 {
                4
            } else {
                0
            };
            // TODO: Something is fishy here, why isn't `width` being used?
            if width == 0 {
                return yaml_parser_set_scanner_error(
                    parser,
                    if directive {
                        "while parsing a %TAG directive"
                    } else {
                        "while parsing a tag"
                    },
                    start_mark,
                    "found an incorrect leading UTF-8 octet",
                );
            }
        } else if octet & 0xC0 != 0x80 {
            return yaml_parser_set_scanner_error(
                parser,
                if directive {
                    "while parsing a %TAG directive"
                } else {
                    "while parsing a tag"
                },
                start_mark,
                "found an incorrect trailing UTF-8 octet",
            );
        }
        string.push(char::from_u32(octet as _).expect("invalid Unicode"));
        SKIP(parser);
        SKIP(parser);
        SKIP(parser);
        width -= 1;
        if !(width != 0) {
            break;
        }
    }
    Ok(())
}

fn yaml_parser_scan_block_scalar(
    parser: &mut yaml_parser_t,
    token: &mut yaml_token_t,
    literal: bool,
) -> Result<(), ScannerError> {
    let mut end_mark: yaml_mark_t;
    let mut string = String::new();
    let mut leading_break = String::new();
    let mut trailing_breaks = String::new();
    let mut chomping: libc::c_int = 0;
    let mut increment: libc::c_int = 0;
    let mut indent: libc::c_int = 0;
    let mut leading_blank: libc::c_int = 0;
    let mut trailing_blank: libc::c_int;
    let start_mark: yaml_mark_t = parser.mark;
    SKIP(parser);
    CACHE(parser, 1_u64)?;

    if CHECK!(parser.buffer, '+') || CHECK!(parser.buffer, '-') {
        chomping = if CHECK!(parser.buffer, '+') { 1 } else { -1 };
        SKIP(parser);
        CACHE(parser, 1_u64)?;
        if IS_DIGIT!(parser.buffer) {
            if CHECK!(parser.buffer, '0') {
                return yaml_parser_set_scanner_error(
                    parser,
                    "while scanning a block scalar",
                    start_mark,
                    "found an indentation indicator equal to 0",
                );
            } else {
                increment = AS_DIGIT!(parser.buffer) as i32;
                SKIP(parser);
            }
        }
    } else if IS_DIGIT!(parser.buffer) {
        if CHECK!(parser.buffer, '0') {
            return yaml_parser_set_scanner_error(
                parser,
                "while scanning a block scalar",
                start_mark,
                "found an indentation indicator equal to 0",
            );
        } else {
            increment = AS_DIGIT!(parser.buffer) as i32;
            SKIP(parser);
            CACHE(parser, 1_u64)?;
            if CHECK!(parser.buffer, '+') || CHECK!(parser.buffer, '-') {
                chomping = if CHECK!(parser.buffer, '+') { 1 } else { -1 };
                SKIP(parser);
            }
        }
    }

    CACHE(parser, 1_u64)?;
    loop {
        if !IS_BLANK!(parser.buffer) {
            break;
        }
        SKIP(parser);
        CACHE(parser, 1_u64)?;
    }

    if CHECK!(parser.buffer, '#') {
        loop {
            if IS_BREAKZ!(parser.buffer) {
                break;
            }
            SKIP(parser);
            CACHE(parser, 1_u64)?;
        }
    }

    if !IS_BREAKZ!(parser.buffer) {
        return yaml_parser_set_scanner_error(
            parser,
            "while scanning a block scalar",
            start_mark,
            "did not find expected comment or line break",
        );
    }

    if IS_BREAK!(parser.buffer) {
        CACHE(parser, 2_u64)?;
        SKIP_LINE(parser);
    }

    end_mark = parser.mark;
    if increment != 0 {
        indent = if parser.indent >= 0 {
            parser.indent + increment
        } else {
            increment
        };
    }
    yaml_parser_scan_block_scalar_breaks(
        parser,
        &mut indent,
        &mut trailing_breaks,
        start_mark,
        &mut end_mark,
    )?;

    CACHE(parser, 1_u64)?;

    loop {
        if !(parser.mark.column as libc::c_int == indent && !IS_Z!(parser.buffer)) {
            break;
        }
        trailing_blank = IS_BLANK!(parser.buffer) as libc::c_int;
        if !literal && leading_break.starts_with('\n') && leading_blank == 0 && trailing_blank == 0
        {
            if trailing_breaks.is_empty() {
                string.push(' ');
            }
            leading_break.clear();
        } else {
            string.push_str(&leading_break);
            leading_break.clear();
        }
        string.push_str(&trailing_breaks);
        trailing_breaks.clear();
        leading_blank = IS_BLANK!(parser.buffer) as libc::c_int;
        while !IS_BREAKZ!(parser.buffer) {
            READ_STRING(parser, &mut string);
            CACHE(parser, 1_u64)?;
        }
        CACHE(parser, 2_u64)?;
        READ_LINE_STRING(parser, &mut leading_break);
        yaml_parser_scan_block_scalar_breaks(
            parser,
            &mut indent,
            &mut trailing_breaks,
            start_mark,
            &mut end_mark,
        )?;
    }

    if chomping != -1 {
        string.push_str(&leading_break);
    }

    if chomping == 1 {
        string.push_str(&trailing_breaks);
    }

    *token = yaml_token_t {
        data: YamlTokenData::Scalar {
            value: string,
            style: if literal {
                YAML_LITERAL_SCALAR_STYLE
            } else {
                YAML_FOLDED_SCALAR_STYLE
            },
        },
        start_mark,
        end_mark,
    };

    Ok(())
}

fn yaml_parser_scan_block_scalar_breaks(
    parser: &mut yaml_parser_t,
    indent: &mut libc::c_int,
    breaks: &mut String,
    start_mark: yaml_mark_t,
    end_mark: &mut yaml_mark_t,
) -> Result<(), ScannerError> {
    let mut max_indent: libc::c_int = 0;
    *end_mark = parser.mark;
    loop {
        CACHE(parser, 1_u64)?;
        while (*indent == 0 || (parser.mark.column as libc::c_int) < *indent)
            && IS_SPACE!(parser.buffer)
        {
            SKIP(parser);
            CACHE(parser, 1_u64)?;
        }
        if parser.mark.column as libc::c_int > max_indent {
            max_indent = parser.mark.column as libc::c_int;
        }
        if (*indent == 0 || (parser.mark.column as libc::c_int) < *indent) && IS_TAB!(parser.buffer)
        {
            return yaml_parser_set_scanner_error(
                parser,
                "while scanning a block scalar",
                start_mark,
                "found a tab character where an indentation space is expected",
            );
        }
        if !IS_BREAK!(parser.buffer) {
            break;
        }
        CACHE(parser, 2_u64)?;
        READ_LINE_STRING(parser, breaks);
        *end_mark = parser.mark;
    }
    if *indent == 0 {
        *indent = max_indent;
        if *indent < parser.indent + 1 {
            *indent = parser.indent + 1;
        }
        if *indent < 1 {
            *indent = 1;
        }
    }
    Ok(())
}

fn yaml_parser_scan_flow_scalar(
    parser: &mut yaml_parser_t,
    token: &mut yaml_token_t,
    single: bool,
) -> Result<(), ScannerError> {
    let mut string = String::new();
    let mut leading_break = String::new();
    let mut trailing_breaks = String::new();
    let mut whitespaces = String::new();
    let mut leading_blanks;

    let start_mark: yaml_mark_t = parser.mark;
    SKIP(parser);
    loop {
        CACHE(parser, 4_u64)?;

        if parser.mark.column == 0_u64
            && (CHECK_AT!(parser.buffer, '-', 0)
                && CHECK_AT!(parser.buffer, '-', 1)
                && CHECK_AT!(parser.buffer, '-', 2)
                || CHECK_AT!(parser.buffer, '.', 0)
                    && CHECK_AT!(parser.buffer, '.', 1)
                    && CHECK_AT!(parser.buffer, '.', 2))
            && IS_BLANKZ_AT!(parser.buffer, 3)
        {
            return yaml_parser_set_scanner_error(
                parser,
                "while scanning a quoted scalar",
                start_mark,
                "found unexpected document indicator",
            );
        } else if IS_Z!(parser.buffer) {
            return yaml_parser_set_scanner_error(
                parser,
                "while scanning a quoted scalar",
                start_mark,
                "found unexpected end of stream",
            );
        } else {
            CACHE(parser, 2_u64)?;
            leading_blanks = false;
            while !IS_BLANKZ!(parser.buffer) {
                if single && CHECK_AT!(parser.buffer, '\'', 0) && CHECK_AT!(parser.buffer, '\'', 1)
                {
                    string.push('\'');
                    SKIP(parser);
                    SKIP(parser);
                } else {
                    if CHECK!(parser.buffer, if single { '\'' } else { '"' }) {
                        break;
                    }
                    if !single && CHECK!(parser.buffer, '\\') && IS_BREAK_AT!(parser.buffer, 1) {
                        CACHE(parser, 3_u64)?;
                        SKIP(parser);
                        SKIP_LINE(parser);
                        leading_blanks = true;
                        break;
                    } else if !single && CHECK!(parser.buffer, '\\') {
                        let mut code_length: size_t = 0_u64;
                        match parser.buffer.get(1).copied().unwrap() {
                            '0' => {
                                string.push('\0');
                            }
                            'a' => {
                                string.push('\x07');
                            }
                            'b' => {
                                string.push('\x08');
                            }
                            't' | '\t' => {
                                string.push('\t');
                            }
                            'n' => {
                                string.push('\n');
                            }
                            'v' => {
                                string.push('\x0B');
                            }
                            'f' => {
                                string.push('\x0C');
                            }
                            'r' => {
                                string.push('\r');
                            }
                            'e' => {
                                string.push('\x1B');
                            }
                            ' ' => {
                                string.push(' ');
                            }
                            '"' => {
                                string.push('"');
                            }
                            '/' => {
                                string.push('/');
                            }
                            '\\' => {
                                string.push('\\');
                            }
                            // NEL (#x85)
                            'N' => {
                                string.push('\u{0085}');
                            }
                            // #xA0
                            '_' => {
                                string.push('\u{00a0}');
                                // string.push('\xC2');
                                // string.push('\xA0');
                            }
                            // LS (#x2028)
                            'L' => {
                                string.push('\u{2028}');
                                // string.push('\xE2');
                                // string.push('\x80');
                                // string.push('\xA8');
                            }
                            // PS (#x2029)
                            'P' => {
                                string.push('\u{2029}');
                                // string.push('\xE2');
                                // string.push('\x80');
                                // string.push('\xA9');
                            }
                            'x' => {
                                code_length = 2_u64;
                            }
                            'u' => {
                                code_length = 4_u64;
                            }
                            'U' => {
                                code_length = 8_u64;
                            }
                            _ => {
                                return yaml_parser_set_scanner_error(
                                    parser,
                                    "while parsing a quoted scalar",
                                    start_mark,
                                    "found unknown escape character",
                                );
                            }
                        }
                        SKIP(parser);
                        SKIP(parser);
                        if code_length != 0 {
                            let mut value: libc::c_uint = 0;
                            let mut k: size_t;
                            CACHE(parser, code_length)?;
                            k = 0_u64;
                            while k < code_length {
                                if !IS_HEX_AT!(parser.buffer, k as usize) {
                                    return yaml_parser_set_scanner_error(
                                        parser,
                                        "while parsing a quoted scalar",
                                        start_mark,
                                        "did not find expected hexdecimal number",
                                    );
                                } else {
                                    value = (value << 4)
                                        .force_add(AS_HEX_AT!(parser.buffer, k as usize));
                                    k = k.force_add(1);
                                }
                            }
                            if let Some(ch) = char::from_u32(value) {
                                string.push(ch);
                            } else {
                                return yaml_parser_set_scanner_error(
                                    parser,
                                    "while parsing a quoted scalar",
                                    start_mark,
                                    "found invalid Unicode character escape code",
                                );
                            }

                            k = 0_u64;
                            while k < code_length {
                                SKIP(parser);
                                k = k.force_add(1);
                            }
                        }
                    } else {
                        READ_STRING(parser, &mut string);
                    }
                }
                CACHE(parser, 2_u64)?;
            }
            CACHE(parser, 1_u64)?;
            if CHECK!(parser.buffer, if single { '\'' } else { '"' }) {
                break;
            }
            CACHE(parser, 1_u64)?;
            while IS_BLANK!(parser.buffer) || IS_BREAK!(parser.buffer) {
                if IS_BLANK!(parser.buffer) {
                    if !leading_blanks {
                        READ_STRING(parser, &mut whitespaces);
                    } else {
                        SKIP(parser);
                    }
                } else {
                    CACHE(parser, 2_u64)?;
                    if !leading_blanks {
                        whitespaces.clear();
                        READ_LINE_STRING(parser, &mut leading_break);
                        leading_blanks = true;
                    } else {
                        READ_LINE_STRING(parser, &mut trailing_breaks);
                    }
                }
                CACHE(parser, 1_u64)?;
            }
            if leading_blanks {
                if leading_break.starts_with('\n') {
                    if trailing_breaks.is_empty() {
                        string.push(' ');
                    } else {
                        string.push_str(&trailing_breaks);
                        trailing_breaks.clear();
                    }
                    leading_break.clear();
                } else {
                    string.push_str(&leading_break);
                    string.push_str(&trailing_breaks);
                    leading_break.clear();
                    trailing_breaks.clear();
                }
            } else {
                string.push_str(&whitespaces);
                whitespaces.clear();
            }
        }
    }

    SKIP(parser);
    let end_mark: yaml_mark_t = parser.mark;
    *token = yaml_token_t {
        data: YamlTokenData::Scalar {
            value: string,
            style: if single {
                YAML_SINGLE_QUOTED_SCALAR_STYLE
            } else {
                YAML_DOUBLE_QUOTED_SCALAR_STYLE
            },
        },
        start_mark,
        end_mark,
    };
    Ok(())
}

fn yaml_parser_scan_plain_scalar(
    parser: &mut yaml_parser_t,
    token: &mut yaml_token_t,
) -> Result<(), ScannerError> {
    let mut end_mark: yaml_mark_t;
    let mut string = String::new();
    let mut leading_break = String::new();
    let mut trailing_breaks = String::new();
    let mut whitespaces = String::new();
    let mut leading_blanks = false;
    let indent: libc::c_int = parser.indent + 1;
    end_mark = parser.mark;
    let start_mark: yaml_mark_t = end_mark;
    loop {
        CACHE(parser, 4_u64)?;
        if parser.mark.column == 0_u64
            && (CHECK_AT!(parser.buffer, '-', 0)
                && CHECK_AT!(parser.buffer, '-', 1)
                && CHECK_AT!(parser.buffer, '-', 2)
                || CHECK_AT!(parser.buffer, '.', 0)
                    && CHECK_AT!(parser.buffer, '.', 1)
                    && CHECK_AT!(parser.buffer, '.', 2))
            && IS_BLANKZ_AT!(parser.buffer, 3)
        {
            break;
        }
        if CHECK!(parser.buffer, '#') {
            break;
        }
        while !IS_BLANKZ!(parser.buffer) {
            if parser.flow_level != 0
                && CHECK!(parser.buffer, ':')
                && (CHECK_AT!(parser.buffer, ',', 1)
                    || CHECK_AT!(parser.buffer, '?', 1)
                    || CHECK_AT!(parser.buffer, '[', 1)
                    || CHECK_AT!(parser.buffer, ']', 1)
                    || CHECK_AT!(parser.buffer, '{', 1)
                    || CHECK_AT!(parser.buffer, '}', 1))
            {
                return yaml_parser_set_scanner_error(
                    parser,
                    "while scanning a plain scalar",
                    start_mark,
                    "found unexpected ':'",
                );
            } else {
                if CHECK!(parser.buffer, ':') && IS_BLANKZ_AT!(parser.buffer, 1)
                    || parser.flow_level != 0
                        && (CHECK!(parser.buffer, ',')
                            || CHECK!(parser.buffer, '[')
                            || CHECK!(parser.buffer, ']')
                            || CHECK!(parser.buffer, '{')
                            || CHECK!(parser.buffer, '}'))
                {
                    break;
                }
                if leading_blanks || !whitespaces.is_empty() {
                    if leading_blanks {
                        if leading_break.starts_with('\n') {
                            if trailing_breaks.is_empty() {
                                string.push(' ');
                            } else {
                                string.push_str(&trailing_breaks);
                                trailing_breaks.clear();
                            }
                            leading_break.clear();
                        } else {
                            string.push_str(&leading_break);
                            string.push_str(&trailing_breaks);
                            leading_break.clear();
                            trailing_breaks.clear();
                        }
                        leading_blanks = false;
                    } else {
                        string.push_str(&whitespaces);
                        whitespaces.clear();
                    }
                }
                READ_STRING(parser, &mut string);
                end_mark = parser.mark;
                CACHE(parser, 2_u64)?;
            }
        }
        if !(IS_BLANK!(parser.buffer) || IS_BREAK!(parser.buffer)) {
            break;
        }
        CACHE(parser, 1_u64)?;

        while IS_BLANK!(parser.buffer) || IS_BREAK!(parser.buffer) {
            if IS_BLANK!(parser.buffer) {
                if leading_blanks
                    && (parser.mark.column as libc::c_int) < indent
                    && IS_TAB!(parser.buffer)
                {
                    return yaml_parser_set_scanner_error(
                        parser,
                        "while scanning a plain scalar",
                        start_mark,
                        "found a tab character that violates indentation",
                    );
                } else if !leading_blanks {
                    READ_STRING(parser, &mut whitespaces);
                } else {
                    SKIP(parser);
                }
            } else {
                CACHE(parser, 2_u64)?;

                if !leading_blanks {
                    whitespaces.clear();
                    READ_LINE_STRING(parser, &mut leading_break);
                    leading_blanks = true;
                } else {
                    READ_LINE_STRING(parser, &mut trailing_breaks);
                }
            }
            CACHE(parser, 1_u64)?;
        }
        if parser.flow_level == 0 && (parser.mark.column as libc::c_int) < indent {
            break;
        }
    }

    *token = yaml_token_t {
        data: YamlTokenData::Scalar {
            value: string,
            style: YAML_PLAIN_SCALAR_STYLE,
        },
        start_mark,
        end_mark,
    };
    if leading_blanks {
        parser.simple_key_allowed = true;
    }

    Ok(())
}
