use alloc::string::String;

use crate::api::OUTPUT_BUFFER_SIZE;
use crate::macros::{
    is_alpha, is_ascii, is_blank, is_blankz, is_bom, is_break, is_printable, is_space,
};
use crate::ops::{ForceAdd as _, ForceMul as _};
use crate::yaml::{size_t, yaml_char_t, YamlEventData};
use crate::{
    libc, yaml_emitter_flush, yaml_emitter_t, yaml_event_delete, yaml_event_t, yaml_scalar_style_t,
    yaml_tag_directive_t, yaml_version_directive_t, YAML_ANY_BREAK, YAML_ANY_ENCODING,
    YAML_ANY_SCALAR_STYLE, YAML_CRLN_BREAK, YAML_CR_BREAK, YAML_DOUBLE_QUOTED_SCALAR_STYLE,
    YAML_EMITTER_ERROR, YAML_EMIT_BLOCK_MAPPING_FIRST_KEY_STATE, YAML_EMIT_BLOCK_MAPPING_KEY_STATE,
    YAML_EMIT_BLOCK_MAPPING_SIMPLE_VALUE_STATE, YAML_EMIT_BLOCK_MAPPING_VALUE_STATE,
    YAML_EMIT_BLOCK_SEQUENCE_FIRST_ITEM_STATE, YAML_EMIT_BLOCK_SEQUENCE_ITEM_STATE,
    YAML_EMIT_DOCUMENT_CONTENT_STATE, YAML_EMIT_DOCUMENT_END_STATE, YAML_EMIT_DOCUMENT_START_STATE,
    YAML_EMIT_END_STATE, YAML_EMIT_FIRST_DOCUMENT_START_STATE,
    YAML_EMIT_FLOW_MAPPING_FIRST_KEY_STATE, YAML_EMIT_FLOW_MAPPING_KEY_STATE,
    YAML_EMIT_FLOW_MAPPING_SIMPLE_VALUE_STATE, YAML_EMIT_FLOW_MAPPING_VALUE_STATE,
    YAML_EMIT_FLOW_SEQUENCE_FIRST_ITEM_STATE, YAML_EMIT_FLOW_SEQUENCE_ITEM_STATE,
    YAML_EMIT_STREAM_START_STATE, YAML_FLOW_MAPPING_STYLE, YAML_FLOW_SEQUENCE_STYLE,
    YAML_FOLDED_SCALAR_STYLE, YAML_LITERAL_SCALAR_STYLE, YAML_LN_BREAK, YAML_PLAIN_SCALAR_STYLE,
    YAML_SINGLE_QUOTED_SCALAR_STYLE, YAML_UTF8_ENCODING,
};
use core::ptr::{self};

unsafe fn FLUSH(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    if emitter.buffer.len() < OUTPUT_BUFFER_SIZE - 5 {
        Ok(())
    } else {
        yaml_emitter_flush(emitter)
    }
}

unsafe fn PUT(emitter: &mut yaml_emitter_t, value: u8) -> Result<(), ()> {
    FLUSH(emitter)?;
    let ch = char::try_from(value).expect("invalid char");
    emitter.buffer.push(ch);
    emitter.column += 1;
    Ok(())
}

unsafe fn PUT_BREAK(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    FLUSH(emitter)?;
    if emitter.line_break == YAML_CR_BREAK {
        emitter.buffer.push('\r');
    } else if emitter.line_break == YAML_LN_BREAK {
        emitter.buffer.push('\n');
    } else if emitter.line_break == YAML_CRLN_BREAK {
        emitter.buffer.push_str("\r\n");
    };
    emitter.column = 0;
    emitter.line += 1;
    Ok(())
}

/// Write UTF-8 charanters from `string` to `emitter` and increment
/// `emitter.column` the appropriate number of times.
unsafe fn WRITE_STR(emitter: &mut yaml_emitter_t, string: &str) -> Result<(), ()> {
    for ch in string.chars() {
        WRITE_CHAR(emitter, ch)?;
    }
    Ok(())
}

unsafe fn WRITE_CHAR(emitter: &mut yaml_emitter_t, ch: char) -> Result<(), ()> {
    FLUSH(emitter)?;
    emitter.buffer.push(ch);
    emitter.column += 1;
    Ok(())
}

unsafe fn WRITE_BREAK_CHAR(emitter: &mut yaml_emitter_t, ch: char) -> Result<(), ()> {
    FLUSH(emitter)?;
    if ch == '\n' {
        _ = PUT_BREAK(emitter);
    } else {
        WRITE_CHAR(emitter, ch)?;
        emitter.column = 0;
        emitter.line += 1;
    }
    Ok(())
}

fn yaml_emitter_set_emitter_error(
    emitter: &mut yaml_emitter_t,
    problem: &'static str,
) -> Result<(), ()> {
    emitter.error = YAML_EMITTER_ERROR;
    emitter.problem = Some(problem);
    Err(())
}

/// Emit an event.
///
/// The event object may be generated using the yaml_parser_parse() function.
/// The emitter takes the responsibility for the event object and destroys its
/// content after it is emitted. The event object is destroyed even if the
/// function fails.
pub unsafe fn yaml_emitter_emit(
    emitter: &mut yaml_emitter_t,
    event: yaml_event_t,
) -> Result<(), ()> {
    emitter.events.push_back(event);
    while let Some(mut event) = yaml_emitter_needs_mode_events(emitter) {
        yaml_emitter_analyze_event(emitter, &event)?;
        yaml_emitter_state_machine(emitter, &event)?;
        yaml_event_delete(&mut event);
    }
    Ok(())
}

fn yaml_emitter_needs_mode_events(emitter: &mut yaml_emitter_t) -> Option<yaml_event_t> {
    let first = emitter.events.front()?;

    let accummulate = match &first.data {
        YamlEventData::DocumentStart { .. } => 1,
        YamlEventData::SequenceStart { .. } => 2,
        YamlEventData::MappingStart { .. } => 3,
        _ => return emitter.events.pop_front(),
    };

    if emitter.events.len() > accummulate {
        return emitter.events.pop_front();
    }

    let mut level = 0;
    for event in &emitter.events {
        match event.data {
            YamlEventData::StreamStart { .. }
            | YamlEventData::DocumentStart { .. }
            | YamlEventData::SequenceStart { .. }
            | YamlEventData::MappingStart { .. } => {
                level += 1;
            }

            YamlEventData::StreamEnd
            | YamlEventData::DocumentEnd { .. }
            | YamlEventData::SequenceEnd
            | YamlEventData::MappingEnd => {
                level -= 1;
            }
            _ => {}
        }

        if level == 0 {
            return emitter.events.pop_front();
        }
    }

    None
}

unsafe fn yaml_emitter_append_tag_directive(
    emitter: &mut yaml_emitter_t,
    value: &yaml_tag_directive_t,
    allow_duplicates: bool,
) -> Result<(), ()> {
    for tag_directive in emitter.tag_directives.iter() {
        if value.handle == tag_directive.handle {
            if allow_duplicates {
                return Ok(());
            }
            return yaml_emitter_set_emitter_error(emitter, "duplicate %TAG directive");
        }
    }
    emitter.tag_directives.push(value.clone());
    Ok(())
}

unsafe fn yaml_emitter_increase_indent(emitter: &mut yaml_emitter_t, flow: bool, indentless: bool) {
    emitter.indents.push(emitter.indent);
    if emitter.indent < 0 {
        emitter.indent = if flow { emitter.best_indent } else { 0 };
    } else if !indentless {
        emitter.indent += emitter.best_indent;
    }
}

unsafe fn yaml_emitter_state_machine(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
) -> Result<(), ()> {
    match emitter.state {
        YAML_EMIT_STREAM_START_STATE => yaml_emitter_emit_stream_start(emitter, event),
        YAML_EMIT_FIRST_DOCUMENT_START_STATE => {
            yaml_emitter_emit_document_start(emitter, event, true)
        }
        YAML_EMIT_DOCUMENT_START_STATE => yaml_emitter_emit_document_start(emitter, event, false),
        YAML_EMIT_DOCUMENT_CONTENT_STATE => yaml_emitter_emit_document_content(emitter, event),
        YAML_EMIT_DOCUMENT_END_STATE => yaml_emitter_emit_document_end(emitter, event),
        YAML_EMIT_FLOW_SEQUENCE_FIRST_ITEM_STATE => {
            yaml_emitter_emit_flow_sequence_item(emitter, event, true)
        }
        YAML_EMIT_FLOW_SEQUENCE_ITEM_STATE => {
            yaml_emitter_emit_flow_sequence_item(emitter, event, false)
        }
        YAML_EMIT_FLOW_MAPPING_FIRST_KEY_STATE => {
            yaml_emitter_emit_flow_mapping_key(emitter, event, true)
        }
        YAML_EMIT_FLOW_MAPPING_KEY_STATE => {
            yaml_emitter_emit_flow_mapping_key(emitter, event, false)
        }
        YAML_EMIT_FLOW_MAPPING_SIMPLE_VALUE_STATE => {
            yaml_emitter_emit_flow_mapping_value(emitter, event, true)
        }
        YAML_EMIT_FLOW_MAPPING_VALUE_STATE => {
            yaml_emitter_emit_flow_mapping_value(emitter, event, false)
        }
        YAML_EMIT_BLOCK_SEQUENCE_FIRST_ITEM_STATE => {
            yaml_emitter_emit_block_sequence_item(emitter, event, true)
        }
        YAML_EMIT_BLOCK_SEQUENCE_ITEM_STATE => {
            yaml_emitter_emit_block_sequence_item(emitter, event, false)
        }
        YAML_EMIT_BLOCK_MAPPING_FIRST_KEY_STATE => {
            yaml_emitter_emit_block_mapping_key(emitter, event, true)
        }
        YAML_EMIT_BLOCK_MAPPING_KEY_STATE => {
            yaml_emitter_emit_block_mapping_key(emitter, event, false)
        }
        YAML_EMIT_BLOCK_MAPPING_SIMPLE_VALUE_STATE => {
            yaml_emitter_emit_block_mapping_value(emitter, event, true)
        }
        YAML_EMIT_BLOCK_MAPPING_VALUE_STATE => {
            yaml_emitter_emit_block_mapping_value(emitter, event, false)
        }
        YAML_EMIT_END_STATE => {
            yaml_emitter_set_emitter_error(emitter, "expected nothing after STREAM-END")
        }
    }
}

unsafe fn yaml_emitter_emit_stream_start(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
) -> Result<(), ()> {
    emitter.open_ended = 0;
    if let YamlEventData::StreamStart { ref encoding } = event.data {
        if emitter.encoding == YAML_ANY_ENCODING {
            emitter.encoding = *encoding;
        }
        if emitter.encoding == YAML_ANY_ENCODING {
            emitter.encoding = YAML_UTF8_ENCODING;
        }
        if emitter.best_indent < 2 || emitter.best_indent > 9 {
            emitter.best_indent = 2;
        }
        if emitter.best_width >= 0 && emitter.best_width <= emitter.best_indent.force_mul(2) {
            emitter.best_width = 80;
        }
        if emitter.best_width < 0 {
            emitter.best_width = libc::c_int::MAX;
        }
        if emitter.line_break == YAML_ANY_BREAK {
            emitter.line_break = YAML_LN_BREAK;
        }
        emitter.indent = -1;
        emitter.line = 0;
        emitter.column = 0;
        emitter.whitespace = true;
        emitter.indention = true;
        if emitter.encoding != YAML_UTF8_ENCODING {
            yaml_emitter_write_bom(emitter)?;
        }
        emitter.state = YAML_EMIT_FIRST_DOCUMENT_START_STATE;
        return Ok(());
    }
    yaml_emitter_set_emitter_error(emitter, "expected STREAM-START")
}

unsafe fn yaml_emitter_emit_document_start(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
    first: bool,
) -> Result<(), ()> {
    if let YamlEventData::DocumentStart {
        version_directive,
        tag_directives,
        implicit,
    } = &event.data
    {
        let (version_directive, tag_directives, implicit) =
            (*version_directive, tag_directives, *implicit);

        let default_tag_directives: [yaml_tag_directive_t; 2] = [
            // TODO: Avoid these heap allocations.
            yaml_tag_directive_t {
                handle: String::from("!"),
                prefix: String::from("!"),
            },
            yaml_tag_directive_t {
                handle: String::from("!!"),
                prefix: String::from("tag:yaml.org,2002:"),
            },
        ];
        let mut implicit = implicit;
        if let Some(version_directive) = version_directive {
            yaml_emitter_analyze_version_directive(emitter, &version_directive)?;
        }
        for tag_directive in tag_directives.iter() {
            yaml_emitter_analyze_tag_directive(emitter, tag_directive)?;
            yaml_emitter_append_tag_directive(emitter, tag_directive, false)?;
        }
        for tag_directive in default_tag_directives.iter() {
            yaml_emitter_append_tag_directive(emitter, tag_directive, true)?;
        }
        if !first || emitter.canonical {
            implicit = false;
        }
        if (version_directive.is_some() || !tag_directives.is_empty()) && emitter.open_ended != 0 {
            yaml_emitter_write_indicator(emitter, "...", true, false, false)?;
            yaml_emitter_write_indent(emitter)?;
        }
        emitter.open_ended = 0;
        if let Some(version_directive) = version_directive {
            implicit = false;
            yaml_emitter_write_indicator(emitter, "%YAML", true, false, false)?;
            if version_directive.minor == 1 {
                yaml_emitter_write_indicator(emitter, "1.1", true, false, false)?;
            } else {
                yaml_emitter_write_indicator(emitter, "1.2", true, false, false)?;
            }
            yaml_emitter_write_indent(emitter)?;
        }
        if !tag_directives.is_empty() {
            implicit = false;
            for tag_directive in tag_directives.iter() {
                yaml_emitter_write_indicator(emitter, "%TAG", true, false, false)?;
                yaml_emitter_write_tag_handle(emitter, &tag_directive.handle)?;
                yaml_emitter_write_tag_content(emitter, &tag_directive.prefix, true)?;
                yaml_emitter_write_indent(emitter)?;
            }
        }
        if yaml_emitter_check_empty_document(emitter) {
            implicit = false;
        }
        if !implicit {
            yaml_emitter_write_indent(emitter)?;
            yaml_emitter_write_indicator(emitter, "---", true, false, false)?;
            if emitter.canonical {
                yaml_emitter_write_indent(emitter)?;
            }
        }
        emitter.state = YAML_EMIT_DOCUMENT_CONTENT_STATE;
        emitter.open_ended = 0;
        return Ok(());
    } else if let YamlEventData::StreamEnd = &event.data {
        if emitter.open_ended == 2 {
            yaml_emitter_write_indicator(emitter, "...", true, false, false)?;
            emitter.open_ended = 0;
            yaml_emitter_write_indent(emitter)?;
        }
        yaml_emitter_flush(emitter)?;
        emitter.state = YAML_EMIT_END_STATE;
        return Ok(());
    }

    yaml_emitter_set_emitter_error(emitter, "expected DOCUMENT-START or STREAM-END")
}

unsafe fn yaml_emitter_emit_document_content(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
) -> Result<(), ()> {
    emitter.states.push(YAML_EMIT_DOCUMENT_END_STATE);
    yaml_emitter_emit_node(emitter, event, true, false, false, false)
}

unsafe fn yaml_emitter_emit_document_end(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
) -> Result<(), ()> {
    if let YamlEventData::DocumentEnd { implicit } = &event.data {
        let implicit = *implicit;
        yaml_emitter_write_indent(emitter)?;
        if !implicit {
            yaml_emitter_write_indicator(emitter, "...", true, false, false)?;
            emitter.open_ended = 0;
            yaml_emitter_write_indent(emitter)?;
        } else if emitter.open_ended == 0 {
            emitter.open_ended = 1;
        }
        yaml_emitter_flush(emitter)?;
        emitter.state = YAML_EMIT_DOCUMENT_START_STATE;
        emitter.tag_directives.clear();
        return Ok(());
    }

    yaml_emitter_set_emitter_error(emitter, "expected DOCUMENT-END")
}

unsafe fn yaml_emitter_emit_flow_sequence_item(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
    first: bool,
) -> Result<(), ()> {
    if first {
        yaml_emitter_write_indicator(emitter, "[", true, true, false)?;
        yaml_emitter_increase_indent(emitter, true, false);
        emitter.flow_level += 1;
    }
    if let YamlEventData::SequenceEnd = &event.data {
        emitter.flow_level -= 1;
        emitter.indent = emitter.indents.pop().unwrap();
        if emitter.canonical && !first {
            yaml_emitter_write_indicator(emitter, ",", false, false, false)?;
            yaml_emitter_write_indent(emitter)?;
        }
        yaml_emitter_write_indicator(emitter, "]", false, false, false)?;
        emitter.state = emitter.states.pop().unwrap();
        return Ok(());
    }
    if !first {
        yaml_emitter_write_indicator(emitter, ",", false, false, false)?;
    }
    if emitter.canonical || emitter.column > emitter.best_width {
        yaml_emitter_write_indent(emitter)?;
    }
    emitter.states.push(YAML_EMIT_FLOW_SEQUENCE_ITEM_STATE);
    yaml_emitter_emit_node(emitter, event, false, true, false, false)
}

unsafe fn yaml_emitter_emit_flow_mapping_key(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
    first: bool,
) -> Result<(), ()> {
    if first {
        yaml_emitter_write_indicator(emitter, "{", true, true, false)?;
        yaml_emitter_increase_indent(emitter, true, false);
        emitter.flow_level += 1;
    }
    if let YamlEventData::MappingEnd = &event.data {
        if emitter.indents.is_empty() {
            return Err(());
        }
        emitter.flow_level -= 1;
        emitter.indent = emitter.indents.pop().unwrap();
        if emitter.canonical && !first {
            yaml_emitter_write_indicator(emitter, ",", false, false, false)?;
            yaml_emitter_write_indent(emitter)?;
        }
        yaml_emitter_write_indicator(emitter, "}", false, false, false)?;
        emitter.state = emitter.states.pop().unwrap();
        return Ok(());
    }
    if !first {
        yaml_emitter_write_indicator(emitter, ",", false, false, false)?;
    }
    if emitter.canonical || emitter.column > emitter.best_width {
        yaml_emitter_write_indent(emitter)?;
    }
    if !emitter.canonical && yaml_emitter_check_simple_key(emitter, event) {
        emitter
            .states
            .push(YAML_EMIT_FLOW_MAPPING_SIMPLE_VALUE_STATE);
        yaml_emitter_emit_node(emitter, event, false, false, true, true)
    } else {
        yaml_emitter_write_indicator(emitter, "?", true, false, false)?;
        emitter.states.push(YAML_EMIT_FLOW_MAPPING_VALUE_STATE);
        yaml_emitter_emit_node(emitter, event, false, false, true, false)
    }
}

unsafe fn yaml_emitter_emit_flow_mapping_value(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
    simple: bool,
) -> Result<(), ()> {
    if simple {
        yaml_emitter_write_indicator(emitter, ":", false, false, false)?;
    } else {
        if emitter.canonical || emitter.column > emitter.best_width {
            yaml_emitter_write_indent(emitter)?;
        }
        yaml_emitter_write_indicator(emitter, ":", true, false, false)?;
    }
    emitter.states.push(YAML_EMIT_FLOW_MAPPING_KEY_STATE);
    yaml_emitter_emit_node(emitter, event, false, false, true, false)
}

unsafe fn yaml_emitter_emit_block_sequence_item(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
    first: bool,
) -> Result<(), ()> {
    if first {
        yaml_emitter_increase_indent(
            emitter,
            false,
            emitter.mapping_context && !emitter.indention,
        );
    }
    if let YamlEventData::SequenceEnd = &event.data {
        emitter.indent = emitter.indents.pop().unwrap();
        emitter.state = emitter.states.pop().unwrap();
        return Ok(());
    }
    yaml_emitter_write_indent(emitter)?;
    yaml_emitter_write_indicator(emitter, "-", true, false, true)?;
    emitter.states.push(YAML_EMIT_BLOCK_SEQUENCE_ITEM_STATE);
    yaml_emitter_emit_node(emitter, event, false, true, false, false)
}

unsafe fn yaml_emitter_emit_block_mapping_key(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
    first: bool,
) -> Result<(), ()> {
    if first {
        yaml_emitter_increase_indent(emitter, false, false);
    }
    if let YamlEventData::MappingEnd = &event.data {
        emitter.indent = emitter.indents.pop().unwrap();
        emitter.state = emitter.states.pop().unwrap();
        return Ok(());
    }
    yaml_emitter_write_indent(emitter)?;
    if yaml_emitter_check_simple_key(emitter, event) {
        emitter
            .states
            .push(YAML_EMIT_BLOCK_MAPPING_SIMPLE_VALUE_STATE);
        yaml_emitter_emit_node(emitter, event, false, false, true, true)
    } else {
        yaml_emitter_write_indicator(emitter, "?", true, false, true)?;
        emitter.states.push(YAML_EMIT_BLOCK_MAPPING_VALUE_STATE);
        yaml_emitter_emit_node(emitter, event, false, false, true, false)
    }
}

unsafe fn yaml_emitter_emit_block_mapping_value(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
    simple: bool,
) -> Result<(), ()> {
    if simple {
        yaml_emitter_write_indicator(emitter, ":", false, false, false)?;
    } else {
        yaml_emitter_write_indent(emitter)?;
        yaml_emitter_write_indicator(emitter, ":", true, false, true)?;
    }
    emitter.states.push(YAML_EMIT_BLOCK_MAPPING_KEY_STATE);
    yaml_emitter_emit_node(emitter, event, false, false, true, false)
}

unsafe fn yaml_emitter_emit_node(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
    root: bool,
    sequence: bool,
    mapping: bool,
    simple_key: bool,
) -> Result<(), ()> {
    emitter.root_context = root;
    emitter.sequence_context = sequence;
    emitter.mapping_context = mapping;
    emitter.simple_key_context = simple_key;

    match event.data {
        YamlEventData::Alias { .. } => yaml_emitter_emit_alias(emitter, event),
        YamlEventData::Scalar { .. } => yaml_emitter_emit_scalar(emitter, event),
        YamlEventData::SequenceStart { .. } => yaml_emitter_emit_sequence_start(emitter, event),
        YamlEventData::MappingStart { .. } => yaml_emitter_emit_mapping_start(emitter, event),
        _ => yaml_emitter_set_emitter_error(
            emitter,
            "expected SCALAR, SEQUENCE-START, MAPPING-START, or ALIAS",
        ),
    }
}

unsafe fn yaml_emitter_emit_alias(
    emitter: &mut yaml_emitter_t,
    _event: &yaml_event_t,
) -> Result<(), ()> {
    yaml_emitter_process_anchor(emitter)?;
    if emitter.simple_key_context {
        PUT(emitter, b' ')?;
    }
    emitter.state = emitter.states.pop().unwrap();
    Ok(())
}

unsafe fn yaml_emitter_emit_scalar(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
) -> Result<(), ()> {
    yaml_emitter_select_scalar_style(emitter, event)?;
    yaml_emitter_process_anchor(emitter)?;
    yaml_emitter_process_tag(emitter)?;
    yaml_emitter_increase_indent(emitter, true, false);
    yaml_emitter_process_scalar(emitter)?;
    emitter.indent = emitter.indents.pop().unwrap();
    emitter.state = emitter.states.pop().unwrap();
    Ok(())
}

unsafe fn yaml_emitter_emit_sequence_start(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
) -> Result<(), ()> {
    yaml_emitter_process_anchor(emitter)?;
    yaml_emitter_process_tag(emitter)?;

    let style = if let YamlEventData::SequenceStart { style, .. } = &event.data {
        *style
    } else {
        unreachable!()
    };

    if emitter.flow_level != 0
        || emitter.canonical
        || style == YAML_FLOW_SEQUENCE_STYLE
        || yaml_emitter_check_empty_sequence(emitter, event)
    {
        emitter.state = YAML_EMIT_FLOW_SEQUENCE_FIRST_ITEM_STATE;
    } else {
        emitter.state = YAML_EMIT_BLOCK_SEQUENCE_FIRST_ITEM_STATE;
    };
    Ok(())
}

unsafe fn yaml_emitter_emit_mapping_start(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
) -> Result<(), ()> {
    yaml_emitter_process_anchor(emitter)?;
    yaml_emitter_process_tag(emitter)?;

    let style = if let YamlEventData::MappingStart { style, .. } = &event.data {
        *style
    } else {
        unreachable!()
    };

    if emitter.flow_level != 0
        || emitter.canonical
        || style == YAML_FLOW_MAPPING_STYLE
        || yaml_emitter_check_empty_mapping(emitter, event)
    {
        emitter.state = YAML_EMIT_FLOW_MAPPING_FIRST_KEY_STATE;
    } else {
        emitter.state = YAML_EMIT_BLOCK_MAPPING_FIRST_KEY_STATE;
    }
    Ok(())
}

unsafe fn yaml_emitter_check_empty_document(_emitter: &yaml_emitter_t) -> bool {
    false
}

unsafe fn yaml_emitter_check_empty_sequence(
    emitter: &yaml_emitter_t,
    event: &yaml_event_t,
) -> bool {
    if emitter.events.len() < 1 {
        return false;
    }
    let start = if let YamlEventData::SequenceStart { .. } = event.data {
        true
    } else {
        false
    };
    let end = if let YamlEventData::SequenceEnd = emitter.events[0].data {
        true
    } else {
        false
    };
    start && end
}

unsafe fn yaml_emitter_check_empty_mapping(emitter: &yaml_emitter_t, event: &yaml_event_t) -> bool {
    if emitter.events.len() < 1 {
        return false;
    }
    let start = if let YamlEventData::MappingStart { .. } = event.data {
        true
    } else {
        false
    };
    let end = if let YamlEventData::MappingEnd = emitter.events[0].data {
        true
    } else {
        false
    };
    start && end
}

unsafe fn yaml_emitter_check_simple_key(emitter: &yaml_emitter_t, event: &yaml_event_t) -> bool {
    let mut length: size_t = 0_u64;

    match event.data {
        YamlEventData::Alias { .. } => {
            length =
                (length as libc::c_ulong).force_add(emitter.anchor_data.anchor_length) as size_t;
        }
        YamlEventData::Scalar { .. } => {
            if emitter.scalar_data.multiline {
                return false;
            }
            length = (length as libc::c_ulong)
                .force_add(emitter.anchor_data.anchor_length)
                .force_add(emitter.tag_data.handle_length)
                .force_add(emitter.tag_data.suffix_length)
                .force_add(emitter.scalar_data.length) as size_t;
        }
        YamlEventData::SequenceStart { .. } => {
            if !yaml_emitter_check_empty_sequence(emitter, event) {
                return false;
            }
            length = (length as libc::c_ulong)
                .force_add(emitter.anchor_data.anchor_length)
                .force_add(emitter.tag_data.handle_length)
                .force_add(emitter.tag_data.suffix_length) as size_t;
        }
        YamlEventData::MappingStart { .. } => {
            if !yaml_emitter_check_empty_mapping(emitter, event) {
                return false;
            }
            length = (length as libc::c_ulong)
                .force_add(emitter.anchor_data.anchor_length)
                .force_add(emitter.tag_data.handle_length)
                .force_add(emitter.tag_data.suffix_length) as size_t;
        }
        _ => return false,
    }

    if length > 128_u64 {
        return false;
    }

    true
}

unsafe fn yaml_emitter_select_scalar_style(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
) -> Result<(), ()> {
    if let YamlEventData::Scalar {
        plain_implicit,
        quoted_implicit,
        style,
        ..
    } = &event.data
    {
        let mut style: yaml_scalar_style_t = *style;
        let no_tag = emitter.tag_data.handle.is_null() && emitter.tag_data.suffix.is_null();
        if no_tag && !*plain_implicit && !*quoted_implicit {
            return yaml_emitter_set_emitter_error(
                emitter,
                "neither tag nor implicit flags are specified",
            );
        }
        if style == YAML_ANY_SCALAR_STYLE {
            style = YAML_PLAIN_SCALAR_STYLE;
        }
        if emitter.canonical {
            style = YAML_DOUBLE_QUOTED_SCALAR_STYLE;
        }
        if emitter.simple_key_context && emitter.scalar_data.multiline {
            style = YAML_DOUBLE_QUOTED_SCALAR_STYLE;
        }
        if style == YAML_PLAIN_SCALAR_STYLE {
            if emitter.flow_level != 0 && !emitter.scalar_data.flow_plain_allowed
                || emitter.flow_level == 0 && !emitter.scalar_data.block_plain_allowed
            {
                style = YAML_SINGLE_QUOTED_SCALAR_STYLE;
            }
            if emitter.scalar_data.length == 0
                && (emitter.flow_level != 0 || emitter.simple_key_context)
            {
                style = YAML_SINGLE_QUOTED_SCALAR_STYLE;
            }
            if no_tag && !*plain_implicit {
                style = YAML_SINGLE_QUOTED_SCALAR_STYLE;
            }
        }
        if style == YAML_SINGLE_QUOTED_SCALAR_STYLE {
            if !emitter.scalar_data.single_quoted_allowed {
                style = YAML_DOUBLE_QUOTED_SCALAR_STYLE;
            }
        }
        if style == YAML_LITERAL_SCALAR_STYLE || style == YAML_FOLDED_SCALAR_STYLE {
            if !emitter.scalar_data.block_allowed
                || emitter.flow_level != 0
                || emitter.simple_key_context
            {
                style = YAML_DOUBLE_QUOTED_SCALAR_STYLE;
            }
        }
        if no_tag && !*quoted_implicit && style != YAML_PLAIN_SCALAR_STYLE {
            emitter.tag_data.handle =
                b"!\0" as *const u8 as *const libc::c_char as *mut yaml_char_t;
            emitter.tag_data.handle_length = 1_u64;
        }
        emitter.scalar_data.style = style;
        Ok(())
    } else {
        unreachable!()
    }
}

unsafe fn yaml_emitter_process_anchor(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    if emitter.anchor_data.anchor.is_null() {
        return Ok(());
    }
    yaml_emitter_write_indicator(
        emitter,
        if emitter.anchor_data.alias { "*" } else { "&" },
        true,
        false,
        false,
    )?;
    yaml_emitter_write_anchor(
        emitter,
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            emitter.anchor_data.anchor,
            emitter.anchor_data.anchor_length as _,
        )),
    )
}

unsafe fn yaml_emitter_process_tag(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    if emitter.tag_data.handle.is_null() && emitter.tag_data.suffix.is_null() {
        return Ok(());
    }
    if !emitter.tag_data.handle.is_null() {
        yaml_emitter_write_tag_handle(
            emitter,
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                emitter.tag_data.handle,
                emitter.tag_data.handle_length as _,
            )),
        )?;
        if !emitter.tag_data.suffix.is_null() {
            yaml_emitter_write_tag_content(
                emitter,
                core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                    emitter.tag_data.suffix,
                    emitter.tag_data.suffix_length as _,
                )),
                false,
            )?;
        }
    } else {
        yaml_emitter_write_indicator(emitter, "!<", true, false, false)?;
        yaml_emitter_write_tag_content(
            emitter,
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                emitter.tag_data.suffix,
                emitter.tag_data.suffix_length as _,
            )),
            false,
        )?;
        yaml_emitter_write_indicator(emitter, ">", false, false, false)?;
    }
    Ok(())
}

unsafe fn yaml_emitter_process_scalar(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    match emitter.scalar_data.style {
        YAML_PLAIN_SCALAR_STYLE => {
            return yaml_emitter_write_plain_scalar(
                emitter,
                core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                    emitter.scalar_data.value,
                    emitter.scalar_data.length as _,
                )),
                !emitter.simple_key_context,
            );
        }
        YAML_SINGLE_QUOTED_SCALAR_STYLE => {
            return yaml_emitter_write_single_quoted_scalar(
                emitter,
                core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                    emitter.scalar_data.value,
                    emitter.scalar_data.length as _,
                )),
                !emitter.simple_key_context,
            );
        }
        YAML_DOUBLE_QUOTED_SCALAR_STYLE => {
            return yaml_emitter_write_double_quoted_scalar(
                emitter,
                core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                    emitter.scalar_data.value,
                    emitter.scalar_data.length as _,
                )),
                !emitter.simple_key_context,
            );
        }
        YAML_LITERAL_SCALAR_STYLE => {
            return yaml_emitter_write_literal_scalar(
                emitter,
                core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                    emitter.scalar_data.value,
                    emitter.scalar_data.length as _,
                )),
            );
        }
        YAML_FOLDED_SCALAR_STYLE => {
            return yaml_emitter_write_folded_scalar(
                emitter,
                core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                    emitter.scalar_data.value,
                    emitter.scalar_data.length as _,
                )),
            );
        }
        _ => {}
    }
    Err(())
}

unsafe fn yaml_emitter_analyze_version_directive(
    emitter: &mut yaml_emitter_t,
    version_directive: &yaml_version_directive_t,
) -> Result<(), ()> {
    if version_directive.major != 1 || version_directive.minor != 1 && version_directive.minor != 2
    {
        return yaml_emitter_set_emitter_error(emitter, "incompatible %YAML directive");
    }
    Ok(())
}

unsafe fn yaml_emitter_analyze_tag_directive(
    emitter: &mut yaml_emitter_t,
    tag_directive: &yaml_tag_directive_t,
) -> Result<(), ()> {
    if tag_directive.handle.is_empty() {
        return yaml_emitter_set_emitter_error(emitter, "tag handle must not be empty");
    }
    if !tag_directive.handle.starts_with('!') {
        return yaml_emitter_set_emitter_error(emitter, "tag handle must start with '!'");
    }
    if !tag_directive.handle.ends_with('!') {
        return yaml_emitter_set_emitter_error(emitter, "tag handle must end with '!'");
    }
    if tag_directive.handle.len() > 2 {
        let tag_content = &tag_directive.handle[1..tag_directive.handle.len() - 1];
        for ch in tag_content.chars() {
            if !IS_ALPHA_CHAR!(ch) {
                return yaml_emitter_set_emitter_error(
                    emitter,
                    "tag handle must contain alphanumerical characters only",
                );
            }
        }
    }

    if tag_directive.prefix.is_empty() {
        return yaml_emitter_set_emitter_error(emitter, "tag prefix must not be empty");
    }

    Ok(())
}

unsafe fn yaml_emitter_analyze_anchor(
    emitter: &mut yaml_emitter_t,
    anchor: &str,
    alias: bool,
) -> Result<(), ()> {
    if anchor.is_empty() {
        return yaml_emitter_set_emitter_error(
            emitter,
            if alias {
                "alias value must not be empty"
            } else {
                "anchor value must not be empty"
            },
        );
    }

    for ch in anchor.chars() {
        if !IS_ALPHA_CHAR!(ch) {
            return yaml_emitter_set_emitter_error(
                emitter,
                if alias {
                    "alias value must contain alphanumerical characters only"
                } else {
                    "anchor value must contain alphanumerical characters only"
                },
            );
        }
    }

    emitter.anchor_data.anchor = anchor.as_ptr();
    emitter.anchor_data.anchor_length = anchor.len() as _;
    emitter.anchor_data.alias = alias;
    Ok(())
}

unsafe fn yaml_emitter_analyze_tag(emitter: &mut yaml_emitter_t, tag: &str) -> Result<(), ()> {
    if tag.is_empty() {
        return yaml_emitter_set_emitter_error(emitter, "tag value must not be empty");
    }

    for tag_directive in emitter.tag_directives.iter() {
        let prefix_len = tag_directive.prefix.len();
        if prefix_len < tag.len() && tag_directive.prefix == tag[0..prefix_len] {
            emitter.tag_data.handle = tag_directive.handle.as_ptr();
            emitter.tag_data.handle_length = tag_directive.handle.len() as _;
            let suffix = &tag[prefix_len..];
            emitter.tag_data.suffix = suffix.as_ptr();
            emitter.tag_data.suffix_length = suffix.len() as _;
            return Ok(());
        }
    }
    emitter.tag_data.suffix = tag.as_ptr();
    emitter.tag_data.suffix_length = tag.len() as _;
    Ok(())
}

unsafe fn yaml_emitter_analyze_scalar(emitter: &mut yaml_emitter_t, value: &str) -> Result<(), ()> {
    let mut block_indicators = false;
    let mut flow_indicators = false;
    let mut line_breaks = false;
    let mut special_characters = false;
    let mut leading_space = false;
    let mut leading_break = false;
    let mut trailing_space = false;
    let mut trailing_break = false;
    let mut break_space = false;
    let mut space_break = false;
    let mut preceded_by_whitespace;
    let mut previous_space = false;
    let mut previous_break = false;

    emitter.scalar_data.value = value.as_ptr();
    emitter.scalar_data.length = value.len() as _;

    if value.is_empty() {
        emitter.scalar_data.multiline = false;
        emitter.scalar_data.flow_plain_allowed = false;
        emitter.scalar_data.block_plain_allowed = true;
        emitter.scalar_data.single_quoted_allowed = true;
        emitter.scalar_data.block_allowed = false;
        return Ok(());
    }

    if value.starts_with("---") || value.starts_with("...") {
        block_indicators = true;
        flow_indicators = true;
    }
    preceded_by_whitespace = true;

    let mut chars = value.chars();
    let mut first = true;

    while let Some(ch) = chars.next() {
        let next = chars.clone().next();
        let followed_by_whitespace = is_blankz(next);
        if first {
            match ch {
                '#' | ',' | '[' | ']' | '{' | '}' | '&' | '*' | '!' | '|' | '>' | '\'' | '"'
                | '%' | '@' | '`' => {
                    flow_indicators = true;
                    block_indicators = true;
                }
                '?' | ':' => {
                    flow_indicators = true;
                    if followed_by_whitespace {
                        block_indicators = true;
                    }
                }
                '-' if followed_by_whitespace => {
                    flow_indicators = true;
                    block_indicators = true;
                }
                _ => {}
            }
        } else {
            match ch {
                ',' | '?' | '[' | ']' | '{' | '}' => {
                    flow_indicators = true;
                }
                ':' => {
                    flow_indicators = true;
                    if followed_by_whitespace {
                        block_indicators = true;
                    }
                }
                '#' if preceded_by_whitespace => {
                    flow_indicators = true;
                    block_indicators = true;
                }
                _ => {}
            }
        }

        if !is_printable(ch) || !is_ascii(ch) && !emitter.unicode {
            special_characters = true;
        }
        if is_break(ch) {
            line_breaks = true;
        }

        if is_space(ch) {
            if first {
                leading_space = true;
            }
            if next.is_none() {
                trailing_space = true;
            }
            if previous_break {
                break_space = true;
            }
            previous_space = true;
            previous_break = false;
        } else if is_break(ch) {
            if first {
                leading_break = true;
            }
            if next.is_none() {
                trailing_break = true;
            }
            if previous_space {
                space_break = true;
            }
            previous_space = false;
            previous_break = true;
        } else {
            previous_space = false;
            previous_break = false;
        }

        preceded_by_whitespace = is_blankz(ch);
        first = false;
    }

    emitter.scalar_data.multiline = line_breaks;
    emitter.scalar_data.flow_plain_allowed = true;
    emitter.scalar_data.block_plain_allowed = true;
    emitter.scalar_data.single_quoted_allowed = true;
    emitter.scalar_data.block_allowed = true;
    if leading_space || leading_break || trailing_space || trailing_break {
        emitter.scalar_data.flow_plain_allowed = false;
        emitter.scalar_data.block_plain_allowed = false;
    }
    if trailing_space {
        emitter.scalar_data.block_allowed = false;
    }
    if break_space {
        emitter.scalar_data.flow_plain_allowed = false;
        emitter.scalar_data.block_plain_allowed = false;
        emitter.scalar_data.single_quoted_allowed = false;
    }
    if space_break || special_characters {
        emitter.scalar_data.flow_plain_allowed = false;
        emitter.scalar_data.block_plain_allowed = false;
        emitter.scalar_data.single_quoted_allowed = false;
        emitter.scalar_data.block_allowed = false;
    }
    if line_breaks {
        emitter.scalar_data.flow_plain_allowed = false;
        emitter.scalar_data.block_plain_allowed = false;
    }
    if flow_indicators {
        emitter.scalar_data.flow_plain_allowed = false;
    }
    if block_indicators {
        emitter.scalar_data.block_plain_allowed = false;
    }
    Ok(())
}

unsafe fn yaml_emitter_analyze_event(
    emitter: &mut yaml_emitter_t,
    event: &yaml_event_t,
) -> Result<(), ()> {
    emitter.anchor_data.anchor = ptr::null_mut::<yaml_char_t>();
    emitter.anchor_data.anchor_length = 0_u64;
    emitter.tag_data.handle = ptr::null_mut::<yaml_char_t>();
    emitter.tag_data.handle_length = 0_u64;
    emitter.tag_data.suffix = ptr::null_mut::<yaml_char_t>();
    emitter.tag_data.suffix_length = 0_u64;
    emitter.scalar_data.value = ptr::null_mut::<yaml_char_t>();
    emitter.scalar_data.length = 0_u64;

    match &event.data {
        YamlEventData::Alias { anchor } => yaml_emitter_analyze_anchor(emitter, anchor, true),
        YamlEventData::Scalar {
            anchor,
            tag,
            value,
            plain_implicit,
            quoted_implicit,
            ..
        } => {
            let (plain_implicit, quoted_implicit) = (*plain_implicit, *quoted_implicit);
            if let Some(anchor) = anchor {
                yaml_emitter_analyze_anchor(emitter, anchor, false)?;
            }
            if tag.is_some() && (emitter.canonical || !plain_implicit && !quoted_implicit) {
                yaml_emitter_analyze_tag(emitter, tag.as_deref().unwrap())?;
            }
            yaml_emitter_analyze_scalar(emitter, value)
        }
        YamlEventData::SequenceStart {
            anchor,
            tag,
            implicit,
            ..
        } => {
            if let Some(anchor) = anchor {
                yaml_emitter_analyze_anchor(emitter, anchor, false)?;
            }
            if tag.is_some() && (emitter.canonical || !*implicit) {
                yaml_emitter_analyze_tag(emitter, tag.as_deref().unwrap())?;
            }
            Ok(())
        }
        YamlEventData::MappingStart {
            anchor,
            tag,
            implicit,
            ..
        } => {
            if let Some(anchor) = anchor {
                yaml_emitter_analyze_anchor(emitter, anchor, false)?;
            }
            if tag.is_some() && (emitter.canonical || !*implicit) {
                yaml_emitter_analyze_tag(emitter, tag.as_deref().unwrap())?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

unsafe fn yaml_emitter_write_bom(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    FLUSH(emitter)?;
    emitter.buffer.push('\u{feff}');
    Ok(())
}

unsafe fn yaml_emitter_write_indent(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    let indent: libc::c_int = if emitter.indent >= 0 {
        emitter.indent
    } else {
        0
    };
    if !emitter.indention
        || emitter.column > indent
        || emitter.column == indent && !emitter.whitespace
    {
        PUT_BREAK(emitter)?;
    }
    while emitter.column < indent {
        PUT(emitter, b' ')?;
    }
    emitter.whitespace = true;
    emitter.indention = true;
    Ok(())
}

unsafe fn yaml_emitter_write_indicator(
    emitter: &mut yaml_emitter_t,
    indicator: &str,
    need_whitespace: bool,
    is_whitespace: bool,
    is_indention: bool,
) -> Result<(), ()> {
    if need_whitespace && !emitter.whitespace {
        PUT(emitter, b' ')?;
    }
    WRITE_STR(emitter, indicator)?;
    emitter.whitespace = is_whitespace;
    emitter.indention = emitter.indention && is_indention;
    Ok(())
}

unsafe fn yaml_emitter_write_anchor(emitter: &mut yaml_emitter_t, value: &str) -> Result<(), ()> {
    WRITE_STR(emitter, value)?;
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

unsafe fn yaml_emitter_write_tag_handle(
    emitter: &mut yaml_emitter_t,
    value: &str,
) -> Result<(), ()> {
    if !emitter.whitespace {
        PUT(emitter, b' ')?;
    }
    WRITE_STR(emitter, value)?;
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

unsafe fn yaml_emitter_write_tag_content(
    emitter: &mut yaml_emitter_t,
    value: &str,
    need_whitespace: bool,
) -> Result<(), ()> {
    if need_whitespace && !emitter.whitespace {
        PUT(emitter, b' ')?;
    }

    for ch in value.chars() {
        if is_alpha(ch) {
            WRITE_CHAR(emitter, ch)?;
            continue;
        }

        match ch {
            ';' | '/' | '?' | ':' | '@' | '&' | '=' | '+' | '$' | ',' | '_' | '.' | '~' | '*'
            | '\'' | '(' | ')' | '[' | ']' => {
                WRITE_CHAR(emitter, ch)?;
                continue;
            }
            _ => {}
        }

        // URI escape
        let mut encode_buffer = [0u8; 4];
        let encoded_char = ch.encode_utf8(&mut encode_buffer);
        for value in encoded_char.bytes() {
            let upper = (value >> 4) + if (value >> 4) < 10 { b'0' } else { b'A' - 10 };
            let lower = (value & 0x0F) + if (value & 0x0F) < 10 { b'0' } else { b'A' - 10 };
            PUT(emitter, b'%')?;
            PUT(emitter, upper)?;
            PUT(emitter, lower)?;
        }
    }

    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

unsafe fn yaml_emitter_write_plain_scalar(
    emitter: &mut yaml_emitter_t,
    value: &str,
    allow_breaks: bool,
) -> Result<(), ()> {
    let mut spaces = false;
    let mut breaks = false;
    if !emitter.whitespace && (!value.is_empty() || emitter.flow_level != 0) {
        PUT(emitter, b' ')?;
    }

    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        let next = chars.clone().next();
        if is_space(ch) {
            if allow_breaks && !spaces && emitter.column > emitter.best_width && !is_space(next) {
                yaml_emitter_write_indent(emitter)?;
            } else {
                WRITE_CHAR(emitter, ch)?;
            }
            spaces = true;
        } else if is_break(ch) {
            if !breaks && ch == '\n' {
                PUT_BREAK(emitter)?;
            }
            WRITE_BREAK_CHAR(emitter, ch)?;
            emitter.indention = true;
            breaks = true;
        } else {
            if breaks {
                yaml_emitter_write_indent(emitter)?;
            }
            WRITE_CHAR(emitter, ch)?;
            emitter.indention = false;
            spaces = false;
            breaks = false;
        }
    }
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

unsafe fn yaml_emitter_write_single_quoted_scalar(
    emitter: &mut yaml_emitter_t,
    value: &str,
    allow_breaks: bool,
) -> Result<(), ()> {
    let mut spaces = false;
    let mut breaks = false;
    yaml_emitter_write_indicator(emitter, "'", true, false, false)?;
    let mut chars = value.chars();
    let mut is_first = true;
    while let Some(ch) = chars.next() {
        let next = chars.clone().next();
        let is_last = next.is_none();

        if is_space(ch) {
            if allow_breaks
                && !spaces
                && emitter.column > emitter.best_width
                && !is_first
                && !is_last
                && !is_space(next)
            {
                yaml_emitter_write_indent(emitter)?;
            } else {
                WRITE_CHAR(emitter, ch)?;
            }
            spaces = true;
        } else if is_break(ch) {
            if !breaks && ch == '\n' {
                PUT_BREAK(emitter)?;
            }
            WRITE_BREAK_CHAR(emitter, ch)?;
            emitter.indention = true;
            breaks = true;
        } else {
            if breaks {
                yaml_emitter_write_indent(emitter)?;
            }
            if ch == '\'' {
                PUT(emitter, b'\'')?;
            }
            WRITE_CHAR(emitter, ch)?;
            emitter.indention = false;
            spaces = false;
            breaks = false;
        }

        is_first = false;
    }
    if breaks {
        yaml_emitter_write_indent(emitter)?;
    }
    yaml_emitter_write_indicator(emitter, "'", false, false, false)?;
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

unsafe fn yaml_emitter_write_double_quoted_scalar(
    emitter: &mut yaml_emitter_t,
    value: &str,
    allow_breaks: bool,
) -> Result<(), ()> {
    let mut spaces = false;
    yaml_emitter_write_indicator(emitter, "\"", true, false, false)?;
    let mut chars = value.chars();
    let mut first = true;
    while let Some(ch) = chars.next() {
        if !is_printable(ch)
            || !emitter.unicode && !is_ascii(ch)
            || is_bom(ch)
            || is_break(ch)
            || ch == '"'
            || ch == '\\'
        {
            PUT(emitter, b'\\')?;
            match ch {
                // TODO: Double check these character mappings.
                '\0' => {
                    PUT(emitter, b'0')?;
                }
                '\x07' => {
                    PUT(emitter, b'a')?;
                }
                '\x08' => {
                    PUT(emitter, b'b')?;
                }
                '\x09' => {
                    PUT(emitter, b't')?;
                }
                '\x0A' => {
                    PUT(emitter, b'n')?;
                }
                '\x0B' => {
                    PUT(emitter, b'v')?;
                }
                '\x0C' => {
                    PUT(emitter, b'f')?;
                }
                '\x0D' => {
                    PUT(emitter, b'r')?;
                }
                '\x1B' => {
                    PUT(emitter, b'e')?;
                }
                '\x22' => {
                    PUT(emitter, b'"')?;
                }
                '\x5C' => {
                    PUT(emitter, b'\\')?;
                }
                '\u{0085}' => {
                    PUT(emitter, b'N')?;
                }
                '\u{00A0}' => {
                    PUT(emitter, b'_')?;
                }
                '\u{2028}' => {
                    PUT(emitter, b'L')?;
                }
                '\u{2029}' => {
                    PUT(emitter, b'P')?;
                }
                _ => {
                    let (prefix, width) = if ch <= '\u{00ff}' {
                        (b'x', 2)
                    } else if ch <= '\u{ffff}' {
                        (b'u', 4)
                    } else {
                        (b'U', 8)
                    };
                    PUT(emitter, prefix)?;
                    let mut k = ((width - 1) * 4) as i32;
                    let value_0 = ch as u32;
                    while k >= 0 {
                        let digit = (value_0 >> k) & 0x0F;
                        let Some(digit_char) = char::from_digit(digit, 16) else {
                            unreachable!("digit out of range")
                        };
                        // The libyaml emitter encodes unicode sequences as uppercase hex.
                        let digit_char = digit_char.to_ascii_uppercase();
                        let digit_byte = digit_char as u8;
                        PUT(emitter, digit_byte)?;
                        k -= 4;
                    }
                }
            }
            spaces = false;
        } else if is_space(ch) {
            if allow_breaks
                && !spaces
                && emitter.column > emitter.best_width
                && !first
                && !chars.clone().next().is_none()
            {
                yaml_emitter_write_indent(emitter)?;
                if is_space(chars.clone().next()) {
                    PUT(emitter, b'\\')?;
                }
            } else {
                WRITE_CHAR(emitter, ch)?;
            }
            spaces = true;
        } else {
            WRITE_CHAR(emitter, ch)?;
            spaces = false;
        }

        first = false;
    }
    yaml_emitter_write_indicator(emitter, "\"", false, false, false)?;
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

unsafe fn yaml_emitter_write_block_scalar_hints(
    emitter: &mut yaml_emitter_t,
    string: &str,
) -> Result<(), ()> {
    let mut chomp_hint: Option<&str> = None;

    let first = string.chars().next();
    if is_space(first) || is_break(first) {
        let Some(indent_hint) = char::from_digit(emitter.best_indent as u32, 10) else {
            unreachable!("emitter.best_indent out of range")
        };
        let mut indent_hint_buffer = [0u8; 1];
        let indent_hint = indent_hint.encode_utf8(&mut indent_hint_buffer);
        yaml_emitter_write_indicator(emitter, indent_hint, false, false, false)?;
    }
    emitter.open_ended = 0;

    if string.is_empty() {
        chomp_hint = Some("-");
    } else {
        let mut chars_rev = string.chars().rev();
        let ch = chars_rev.next();
        let next = chars_rev.next();

        if !is_break(ch) {
            chomp_hint = Some("-");
        } else if next.is_none() {
            chomp_hint = Some("+");
            emitter.open_ended = 2;
        } else {
            if is_break(next) {
                chomp_hint = Some("+");
                emitter.open_ended = 2;
            }
        }
    }

    if let Some(chomp_hint) = chomp_hint {
        yaml_emitter_write_indicator(emitter, chomp_hint, false, false, false)?;
    }
    Ok(())
}

unsafe fn yaml_emitter_write_literal_scalar(
    emitter: &mut yaml_emitter_t,
    value: &str,
) -> Result<(), ()> {
    let mut breaks = true;
    yaml_emitter_write_indicator(emitter, "|", true, false, false)?;
    yaml_emitter_write_block_scalar_hints(emitter, value)?;
    PUT_BREAK(emitter)?;
    emitter.indention = true;
    emitter.whitespace = true;
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if is_break(ch) {
            WRITE_BREAK_CHAR(emitter, ch)?;
            emitter.indention = true;
            breaks = true;
        } else {
            if breaks {
                yaml_emitter_write_indent(emitter)?;
            }
            WRITE_CHAR(emitter, ch)?;
            emitter.indention = false;
            breaks = false;
        }
    }
    Ok(())
}

unsafe fn yaml_emitter_write_folded_scalar(
    emitter: &mut yaml_emitter_t,
    value: &str,
) -> Result<(), ()> {
    let mut breaks = true;
    let mut leading_spaces = true;
    yaml_emitter_write_indicator(emitter, ">", true, false, false)?;
    yaml_emitter_write_block_scalar_hints(emitter, value)?;
    PUT_BREAK(emitter)?;
    emitter.indention = true;
    emitter.whitespace = true;

    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if is_break(ch) {
            if !breaks && !leading_spaces && ch == '\n' {
                let mut skip_breaks = chars.clone();
                while is_break(skip_breaks.next()) {}
                if !is_blankz(skip_breaks.next()) {
                    PUT_BREAK(emitter)?;
                }
            }
            WRITE_BREAK_CHAR(emitter, ch)?;
            emitter.indention = true;
            breaks = true;
        } else {
            if breaks {
                yaml_emitter_write_indent(emitter)?;
                leading_spaces = is_blank(ch);
            }
            if !breaks
                && is_space(ch)
                && !is_space(chars.clone().next())
                && emitter.column > emitter.best_width
            {
                yaml_emitter_write_indent(emitter)?;
            } else {
                WRITE_CHAR(emitter, ch)?;
            }
            emitter.indention = false;
            breaks = false;
        }
    }
    Ok(())
}
