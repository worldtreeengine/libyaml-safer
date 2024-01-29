use crate::api::{yaml_free, yaml_strdup};
use crate::externs::{strcmp, strlen, strncmp};
use crate::ops::{ForceAdd as _, ForceMul as _};
use crate::yaml::{size_t, yaml_char_t, yaml_string_t, YamlEventData};
use crate::{
    libc, yaml_emitter_flush, yaml_emitter_t, yaml_event_delete, yaml_event_t, yaml_scalar_style_t,
    yaml_tag_directive_t, yaml_version_directive_t, PointerExt, YAML_ANY_BREAK, YAML_ANY_ENCODING,
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
    if emitter.buffer.pointer.wrapping_offset(5_isize) < emitter.buffer.end {
        Ok(())
    } else {
        yaml_emitter_flush(emitter)
    }
}

unsafe fn PUT(emitter: &mut yaml_emitter_t, value: u8) -> Result<(), ()> {
    FLUSH(emitter)?;
    let p = &mut emitter.buffer.pointer;
    let old_p = *p;
    *p = (*p).wrapping_offset(1);
    *old_p = value;
    emitter.column += 1;
    Ok(())
}

unsafe fn PUT_BREAK(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    FLUSH(emitter)?;
    if emitter.line_break == YAML_CR_BREAK {
        let p = &mut emitter.buffer.pointer;
        let old_p = *p;
        *p = (*p).wrapping_offset(1);
        *old_p = b'\r';
    } else if emitter.line_break == YAML_LN_BREAK {
        let p = &mut emitter.buffer.pointer;
        let old_p = *p;
        *p = (*p).wrapping_offset(1);
        *old_p = b'\n';
    } else if emitter.line_break == YAML_CRLN_BREAK {
        let p = &mut emitter.buffer.pointer;
        let old_p = *p;
        *p = (*p).wrapping_offset(1);
        *old_p = b'\r';
        let p = &mut emitter.buffer.pointer;
        let old_p = *p;
        *p = (*p).wrapping_offset(1);
        *old_p = b'\n';
    };
    emitter.column = 0;
    emitter.line += 1;
    Ok(())
}

unsafe fn WRITE(emitter: &mut yaml_emitter_t, string: &mut yaml_string_t) -> Result<(), ()> {
    FLUSH(emitter)?;
    COPY!(emitter.buffer, *string);
    emitter.column += 1;
    Ok(())
}

unsafe fn WRITE_BREAK(emitter: &mut yaml_emitter_t, string: &mut yaml_string_t) -> Result<(), ()> {
    FLUSH(emitter)?;
    if CHECK!(*string, b'\n') {
        let _ = PUT_BREAK(emitter);
        string.pointer = string.pointer.wrapping_offset(1);
    } else {
        COPY!(emitter.buffer, *string);
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
    let mut copy = yaml_tag_directive_t {
        handle: ptr::null_mut::<yaml_char_t>(),
        prefix: ptr::null_mut::<yaml_char_t>(),
    };
    for tag_directive in emitter.tag_directives.iter() {
        if strcmp(
            value.handle as *mut libc::c_char,
            tag_directive.handle as *mut libc::c_char,
        ) == 0
        {
            if allow_duplicates {
                return Ok(());
            }
            return yaml_emitter_set_emitter_error(emitter, "duplicate %TAG directive");
        }
    }
    copy.handle = yaml_strdup(value.handle);
    copy.prefix = yaml_strdup(value.prefix);
    emitter.tag_directives.push(copy);
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

        let mut default_tag_directives: [yaml_tag_directive_t; 3] = [
            yaml_tag_directive_t {
                handle: b"!\0" as *const u8 as *const libc::c_char as *mut yaml_char_t,
                prefix: b"!\0" as *const u8 as *const libc::c_char as *mut yaml_char_t,
            },
            yaml_tag_directive_t {
                handle: b"!!\0" as *const u8 as *const libc::c_char as *mut yaml_char_t,
                prefix: b"tag:yaml.org,2002:\0" as *const u8 as *const libc::c_char
                    as *mut yaml_char_t,
            },
            yaml_tag_directive_t {
                handle: ptr::null_mut::<yaml_char_t>(),
                prefix: ptr::null_mut::<yaml_char_t>(),
            },
        ];
        let mut tag_directive: *mut yaml_tag_directive_t;
        let mut implicit = implicit;
        if !version_directive.is_null() {
            yaml_emitter_analyze_version_directive(emitter, &*version_directive)?;
        }
        for tag_directive in tag_directives.iter() {
            yaml_emitter_analyze_tag_directive(emitter, &*tag_directive)?;
            yaml_emitter_append_tag_directive(emitter, &*tag_directive, false)?;
        }
        tag_directive = default_tag_directives.as_mut_ptr();
        while !(*tag_directive).handle.is_null() {
            yaml_emitter_append_tag_directive(emitter, &*tag_directive, true)?;
            tag_directive = tag_directive.wrapping_offset(1);
        }
        if !first || emitter.canonical {
            implicit = false;
        }
        if (!version_directive.is_null() || !tag_directives.is_empty()) && emitter.open_ended != 0 {
            yaml_emitter_write_indicator(
                emitter,
                b"...\0" as *const u8 as *const libc::c_char,
                true,
                false,
                false,
            )?;
            yaml_emitter_write_indent(emitter)?;
        }
        emitter.open_ended = 0;
        if !version_directive.is_null() {
            implicit = false;
            yaml_emitter_write_indicator(
                emitter,
                b"%YAML\0" as *const u8 as *const libc::c_char,
                true,
                false,
                false,
            )?;
            if (*version_directive).minor == 1 {
                yaml_emitter_write_indicator(
                    emitter,
                    b"1.1\0" as *const u8 as *const libc::c_char,
                    true,
                    false,
                    false,
                )?;
            } else {
                yaml_emitter_write_indicator(
                    emitter,
                    b"1.2\0" as *const u8 as *const libc::c_char,
                    true,
                    false,
                    false,
                )?;
            }
            yaml_emitter_write_indent(emitter)?;
        }
        if !tag_directives.is_empty() {
            implicit = false;
            for tag_directive in tag_directives.iter() {
                yaml_emitter_write_indicator(
                    emitter,
                    b"%TAG\0" as *const u8 as *const libc::c_char,
                    true,
                    false,
                    false,
                )?;
                yaml_emitter_write_tag_handle(
                    emitter,
                    (*tag_directive).handle,
                    strlen((*tag_directive).handle as *mut libc::c_char),
                )?;
                yaml_emitter_write_tag_content(
                    emitter,
                    (*tag_directive).prefix,
                    strlen((*tag_directive).prefix as *mut libc::c_char),
                    true,
                )?;
                yaml_emitter_write_indent(emitter)?;
            }
        }
        if yaml_emitter_check_empty_document(emitter) {
            implicit = false;
        }
        if !implicit {
            yaml_emitter_write_indent(emitter)?;
            yaml_emitter_write_indicator(
                emitter,
                b"---\0" as *const u8 as *const libc::c_char,
                true,
                false,
                false,
            )?;
            if emitter.canonical {
                yaml_emitter_write_indent(emitter)?;
            }
        }
        emitter.state = YAML_EMIT_DOCUMENT_CONTENT_STATE;
        emitter.open_ended = 0;
        return Ok(());
    } else if let YamlEventData::StreamEnd = &event.data {
        if emitter.open_ended == 2 {
            yaml_emitter_write_indicator(
                emitter,
                b"...\0" as *const u8 as *const libc::c_char,
                true,
                false,
                false,
            )?;
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
            yaml_emitter_write_indicator(
                emitter,
                b"...\0" as *const u8 as *const libc::c_char,
                true,
                false,
                false,
            )?;
            emitter.open_ended = 0;
            yaml_emitter_write_indent(emitter)?;
        } else if emitter.open_ended == 0 {
            emitter.open_ended = 1;
        }
        yaml_emitter_flush(emitter)?;
        emitter.state = YAML_EMIT_DOCUMENT_START_STATE;
        while let Some(tag_directive) = emitter.tag_directives.pop() {
            yaml_free(tag_directive.handle as *mut libc::c_void);
            yaml_free(tag_directive.prefix as *mut libc::c_void);
        }
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
        yaml_emitter_write_indicator(
            emitter,
            b"[\0" as *const u8 as *const libc::c_char,
            true,
            true,
            false,
        )?;
        yaml_emitter_increase_indent(emitter, true, false);
        emitter.flow_level += 1;
    }
    if let YamlEventData::SequenceEnd = &event.data {
        emitter.flow_level -= 1;
        emitter.indent = emitter.indents.pop().unwrap();
        if emitter.canonical && !first {
            yaml_emitter_write_indicator(
                emitter,
                b",\0" as *const u8 as *const libc::c_char,
                false,
                false,
                false,
            )?;
            yaml_emitter_write_indent(emitter)?;
        }
        yaml_emitter_write_indicator(
            emitter,
            b"]\0" as *const u8 as *const libc::c_char,
            false,
            false,
            false,
        )?;
        emitter.state = emitter.states.pop().unwrap();
        return Ok(());
    }
    if !first {
        yaml_emitter_write_indicator(
            emitter,
            b",\0" as *const u8 as *const libc::c_char,
            false,
            false,
            false,
        )?;
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
        yaml_emitter_write_indicator(
            emitter,
            b"{\0" as *const u8 as *const libc::c_char,
            true,
            true,
            false,
        )?;
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
            yaml_emitter_write_indicator(
                emitter,
                b",\0" as *const u8 as *const libc::c_char,
                false,
                false,
                false,
            )?;
            yaml_emitter_write_indent(emitter)?;
        }
        yaml_emitter_write_indicator(
            emitter,
            b"}\0" as *const u8 as *const libc::c_char,
            false,
            false,
            false,
        )?;
        emitter.state = emitter.states.pop().unwrap();
        return Ok(());
    }
    if !first {
        yaml_emitter_write_indicator(
            emitter,
            b",\0" as *const u8 as *const libc::c_char,
            false,
            false,
            false,
        )?;
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
        yaml_emitter_write_indicator(
            emitter,
            b"?\0" as *const u8 as *const libc::c_char,
            true,
            false,
            false,
        )?;
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
        yaml_emitter_write_indicator(
            emitter,
            b":\0" as *const u8 as *const libc::c_char,
            false,
            false,
            false,
        )?;
    } else {
        if emitter.canonical || emitter.column > emitter.best_width {
            yaml_emitter_write_indent(emitter)?;
        }
        yaml_emitter_write_indicator(
            emitter,
            b":\0" as *const u8 as *const libc::c_char,
            true,
            false,
            false,
        )?;
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
    yaml_emitter_write_indicator(
        emitter,
        b"-\0" as *const u8 as *const libc::c_char,
        true,
        false,
        true,
    )?;
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
        yaml_emitter_write_indicator(
            emitter,
            b"?\0" as *const u8 as *const libc::c_char,
            true,
            false,
            true,
        )?;
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
        yaml_emitter_write_indicator(
            emitter,
            b":\0" as *const u8 as *const libc::c_char,
            false,
            false,
            false,
        )?;
    } else {
        yaml_emitter_write_indent(emitter)?;
        yaml_emitter_write_indicator(
            emitter,
            b":\0" as *const u8 as *const libc::c_char,
            true,
            false,
            true,
        )?;
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
        if emitter.anchor_data.alias {
            b"*\0" as *const u8 as *const libc::c_char
        } else {
            b"&\0" as *const u8 as *const libc::c_char
        },
        true,
        false,
        false,
    )?;
    yaml_emitter_write_anchor(
        emitter,
        emitter.anchor_data.anchor,
        emitter.anchor_data.anchor_length,
    )
}

unsafe fn yaml_emitter_process_tag(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    if emitter.tag_data.handle.is_null() && emitter.tag_data.suffix.is_null() {
        return Ok(());
    }
    if !emitter.tag_data.handle.is_null() {
        yaml_emitter_write_tag_handle(
            emitter,
            emitter.tag_data.handle,
            emitter.tag_data.handle_length,
        )?;
        if !emitter.tag_data.suffix.is_null() {
            yaml_emitter_write_tag_content(
                emitter,
                emitter.tag_data.suffix,
                emitter.tag_data.suffix_length,
                false,
            )?;
        }
    } else {
        yaml_emitter_write_indicator(
            emitter,
            b"!<\0" as *const u8 as *const libc::c_char,
            true,
            false,
            false,
        )?;
        yaml_emitter_write_tag_content(
            emitter,
            emitter.tag_data.suffix,
            emitter.tag_data.suffix_length,
            false,
        )?;
        yaml_emitter_write_indicator(
            emitter,
            b">\0" as *const u8 as *const libc::c_char,
            false,
            false,
            false,
        )?;
    }
    Ok(())
}

unsafe fn yaml_emitter_process_scalar(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    match emitter.scalar_data.style {
        YAML_PLAIN_SCALAR_STYLE => {
            return yaml_emitter_write_plain_scalar(
                emitter,
                emitter.scalar_data.value,
                emitter.scalar_data.length,
                !emitter.simple_key_context,
            );
        }
        YAML_SINGLE_QUOTED_SCALAR_STYLE => {
            return yaml_emitter_write_single_quoted_scalar(
                emitter,
                emitter.scalar_data.value,
                emitter.scalar_data.length,
                !emitter.simple_key_context,
            );
        }
        YAML_DOUBLE_QUOTED_SCALAR_STYLE => {
            return yaml_emitter_write_double_quoted_scalar(
                emitter,
                emitter.scalar_data.value,
                emitter.scalar_data.length,
                !emitter.simple_key_context,
            );
        }
        YAML_LITERAL_SCALAR_STYLE => {
            return yaml_emitter_write_literal_scalar(
                emitter,
                emitter.scalar_data.value,
                emitter.scalar_data.length,
            );
        }
        YAML_FOLDED_SCALAR_STYLE => {
            return yaml_emitter_write_folded_scalar(
                emitter,
                emitter.scalar_data.value,
                emitter.scalar_data.length,
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
    let handle_length: size_t = strlen(tag_directive.handle as *mut libc::c_char);
    let prefix_length: size_t = strlen(tag_directive.prefix as *mut libc::c_char);
    let mut handle = STRING_ASSIGN!(tag_directive.handle, handle_length);
    let prefix = STRING_ASSIGN!(tag_directive.prefix, prefix_length);
    if handle.start == handle.end {
        return yaml_emitter_set_emitter_error(emitter, "tag handle must not be empty");
    }
    if *handle.start != b'!' {
        return yaml_emitter_set_emitter_error(emitter, "tag handle must start with '!'");
    }
    if *handle.end.wrapping_offset(-1_isize) != b'!' {
        return yaml_emitter_set_emitter_error(emitter, "tag handle must end with '!'");
    }
    handle.pointer = handle.pointer.wrapping_offset(1);
    while handle.pointer < handle.end.wrapping_offset(-1_isize) {
        if !IS_ALPHA!(handle) {
            return yaml_emitter_set_emitter_error(
                emitter,
                "tag handle must contain alphanumerical characters only",
            );
        }
        MOVE!(handle);
    }
    if prefix.start == prefix.end {
        return yaml_emitter_set_emitter_error(emitter, "tag prefix must not be empty");
    }
    Ok(())
}

unsafe fn yaml_emitter_analyze_anchor(
    emitter: &mut yaml_emitter_t,
    anchor: *mut yaml_char_t,
    alias: bool,
) -> Result<(), ()> {
    let anchor_length: size_t = strlen(anchor as *mut libc::c_char);
    let mut string = STRING_ASSIGN!(anchor, anchor_length);
    if string.start == string.end {
        return yaml_emitter_set_emitter_error(
            emitter,
            if alias {
                "alias value must not be empty"
            } else {
                "anchor value must not be empty"
            },
        );
    }
    while string.pointer != string.end {
        if !IS_ALPHA!(string) {
            return yaml_emitter_set_emitter_error(
                emitter,
                if alias {
                    "alias value must contain alphanumerical characters only"
                } else {
                    "anchor value must contain alphanumerical characters only"
                },
            );
        }
        MOVE!(string);
    }
    emitter.anchor_data.anchor = string.start;
    emitter.anchor_data.anchor_length = string.end.c_offset_from(string.start) as size_t;
    emitter.anchor_data.alias = alias;
    Ok(())
}

unsafe fn yaml_emitter_analyze_tag(
    emitter: &mut yaml_emitter_t,
    tag: *mut yaml_char_t,
) -> Result<(), ()> {
    let tag_length: size_t = strlen(tag as *mut libc::c_char);
    let string = STRING_ASSIGN!(tag, tag_length);
    if string.start == string.end {
        return yaml_emitter_set_emitter_error(emitter, "tag value must not be empty");
    }
    for tag_directive in emitter.tag_directives.iter() {
        let prefix_length: size_t = strlen(tag_directive.prefix as *mut libc::c_char);
        if prefix_length < string.end.c_offset_from(string.start) as size_t
            && strncmp(
                (*tag_directive).prefix as *mut libc::c_char,
                string.start as *mut libc::c_char,
                prefix_length,
            ) == 0
        {
            emitter.tag_data.handle = (*tag_directive).handle;
            emitter.tag_data.handle_length = strlen((*tag_directive).handle as *mut libc::c_char);
            emitter.tag_data.suffix = string.start.wrapping_offset(prefix_length as isize);
            emitter.tag_data.suffix_length = (string.end.c_offset_from(string.start)
                as libc::c_ulong)
                .wrapping_sub(prefix_length);
            return Ok(());
        }
    }
    emitter.tag_data.suffix = string.start;
    emitter.tag_data.suffix_length = string.end.c_offset_from(string.start) as size_t;
    Ok(())
}

unsafe fn yaml_emitter_analyze_scalar(
    emitter: &mut yaml_emitter_t,
    value: *mut yaml_char_t,
    length: size_t,
) -> Result<(), ()> {
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
    let mut followed_by_whitespace;
    let mut previous_space = false;
    let mut previous_break = false;
    let mut string = STRING_ASSIGN!(value, length);
    emitter.scalar_data.value = value;
    emitter.scalar_data.length = length;
    if string.start == string.end {
        emitter.scalar_data.multiline = false;
        emitter.scalar_data.flow_plain_allowed = false;
        emitter.scalar_data.block_plain_allowed = true;
        emitter.scalar_data.single_quoted_allowed = true;
        emitter.scalar_data.block_allowed = false;
        return Ok(());
    }
    if CHECK_AT!(string, b'-', 0) && CHECK_AT!(string, b'-', 1) && CHECK_AT!(string, b'-', 2)
        || CHECK_AT!(string, b'.', 0) && CHECK_AT!(string, b'.', 1) && CHECK_AT!(string, b'.', 2)
    {
        block_indicators = true;
        flow_indicators = true;
    }
    preceded_by_whitespace = true;
    followed_by_whitespace = IS_BLANKZ_AT!(string, WIDTH!(string));
    while string.pointer != string.end {
        if string.start == string.pointer {
            if CHECK!(string, b'#')
                || CHECK!(string, b',')
                || CHECK!(string, b'[')
                || CHECK!(string, b']')
                || CHECK!(string, b'{')
                || CHECK!(string, b'}')
                || CHECK!(string, b'&')
                || CHECK!(string, b'*')
                || CHECK!(string, b'!')
                || CHECK!(string, b'|')
                || CHECK!(string, b'>')
                || CHECK!(string, b'\'')
                || CHECK!(string, b'"')
                || CHECK!(string, b'%')
                || CHECK!(string, b'@')
                || CHECK!(string, b'`')
            {
                flow_indicators = true;
                block_indicators = true;
            }
            if CHECK!(string, b'?') || CHECK!(string, b':') {
                flow_indicators = true;
                if followed_by_whitespace {
                    block_indicators = true;
                }
            }
            if CHECK!(string, b'-') && followed_by_whitespace {
                flow_indicators = true;
                block_indicators = true;
            }
        } else {
            if CHECK!(string, b',')
                || CHECK!(string, b'?')
                || CHECK!(string, b'[')
                || CHECK!(string, b']')
                || CHECK!(string, b'{')
                || CHECK!(string, b'}')
            {
                flow_indicators = true;
            }
            if CHECK!(string, b':') {
                flow_indicators = true;
                if followed_by_whitespace {
                    block_indicators = true;
                }
            }
            if CHECK!(string, b'#') && preceded_by_whitespace {
                flow_indicators = true;
                block_indicators = true;
            }
        }
        if !IS_PRINTABLE!(string) || !IS_ASCII!(string) && !emitter.unicode {
            special_characters = true;
        }
        if IS_BREAK!(string) {
            line_breaks = true;
        }
        if IS_SPACE!(string) {
            if string.start == string.pointer {
                leading_space = true;
            }
            if string.pointer.wrapping_offset(WIDTH!(string) as isize) == string.end {
                trailing_space = true;
            }
            if previous_break {
                break_space = true;
            }
            previous_space = true;
            previous_break = false;
        } else if IS_BREAK!(string) {
            if string.start == string.pointer {
                leading_break = true;
            }
            if string.pointer.wrapping_offset(WIDTH!(string) as isize) == string.end {
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
        preceded_by_whitespace = IS_BLANKZ!(string);
        MOVE!(string);
        if string.pointer != string.end {
            followed_by_whitespace = IS_BLANKZ_AT!(string, WIDTH!(string));
        }
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
        YamlEventData::Alias { anchor } => yaml_emitter_analyze_anchor(emitter, *anchor, true),
        YamlEventData::Scalar {
            anchor,
            tag,
            value,
            length,
            plain_implicit,
            quoted_implicit,
            ..
        } => {
            let (anchor, tag, value, length, plain_implicit, quoted_implicit) = (
                *anchor,
                *tag,
                *value,
                *length,
                *plain_implicit,
                *quoted_implicit,
            );
            if !anchor.is_null() {
                yaml_emitter_analyze_anchor(emitter, anchor, false)?;
            }
            if !tag.is_null() && (emitter.canonical || !plain_implicit && !quoted_implicit) {
                yaml_emitter_analyze_tag(emitter, tag)?;
            }
            yaml_emitter_analyze_scalar(emitter, value, length)
        }
        YamlEventData::SequenceStart {
            anchor,
            tag,
            implicit,
            ..
        } => {
            let (anchor, tag, implicit) = (*anchor, *tag, *implicit);

            if !anchor.is_null() {
                yaml_emitter_analyze_anchor(emitter, anchor, false)?;
            }
            if !tag.is_null() && (emitter.canonical || !implicit) {
                yaml_emitter_analyze_tag(emitter, tag)?;
            }
            Ok(())
        }
        YamlEventData::MappingStart {
            anchor,
            tag,
            implicit,
            ..
        } => {
            let (anchor, tag, implicit) = (*anchor, *tag, *implicit);
            if !anchor.is_null() {
                yaml_emitter_analyze_anchor(emitter, anchor, false)?;
            }
            if !tag.is_null() && (emitter.canonical || !implicit) {
                yaml_emitter_analyze_tag(emitter, tag)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

unsafe fn yaml_emitter_write_bom(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    FLUSH(emitter)?;
    let p = &mut emitter.buffer.pointer;
    let old_pointer = *p;
    *p = (*p).wrapping_offset(1);
    *old_pointer = b'\xEF';
    let p = &mut emitter.buffer.pointer;
    let old_pointer = *p;
    *p = (*p).wrapping_offset(1);
    *old_pointer = b'\xBB';
    let p = &mut emitter.buffer.pointer;
    let old_pointer = *p;
    *p = (*p).wrapping_offset(1);
    *old_pointer = b'\xBF';
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
    indicator: *const libc::c_char,
    need_whitespace: bool,
    is_whitespace: bool,
    is_indention: bool,
) -> Result<(), ()> {
    let indicator_length: size_t = strlen(indicator);
    let mut string = STRING_ASSIGN!(indicator as *mut yaml_char_t, indicator_length);
    if need_whitespace && !emitter.whitespace {
        PUT(emitter, b' ')?;
    }
    while string.pointer != string.end {
        WRITE(emitter, &mut string)?;
    }
    emitter.whitespace = is_whitespace;
    emitter.indention = emitter.indention && is_indention;
    Ok(())
}

unsafe fn yaml_emitter_write_anchor(
    emitter: &mut yaml_emitter_t,
    value: *mut yaml_char_t,
    length: size_t,
) -> Result<(), ()> {
    let mut string = STRING_ASSIGN!(value, length);
    while string.pointer != string.end {
        WRITE(emitter, &mut string)?;
    }
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

unsafe fn yaml_emitter_write_tag_handle(
    emitter: &mut yaml_emitter_t,
    value: *mut yaml_char_t,
    length: size_t,
) -> Result<(), ()> {
    let mut string = STRING_ASSIGN!(value, length);
    if !emitter.whitespace {
        PUT(emitter, b' ')?;
    }
    while string.pointer != string.end {
        WRITE(emitter, &mut string)?;
    }
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

unsafe fn yaml_emitter_write_tag_content(
    emitter: &mut yaml_emitter_t,
    value: *mut yaml_char_t,
    length: size_t,
    need_whitespace: bool,
) -> Result<(), ()> {
    let mut string = STRING_ASSIGN!(value, length);
    if need_whitespace && !emitter.whitespace {
        PUT(emitter, b' ')?;
    }
    while string.pointer != string.end {
        if IS_ALPHA!(string)
            || CHECK!(string, b';')
            || CHECK!(string, b'/')
            || CHECK!(string, b'?')
            || CHECK!(string, b':')
            || CHECK!(string, b'@')
            || CHECK!(string, b'&')
            || CHECK!(string, b'=')
            || CHECK!(string, b'+')
            || CHECK!(string, b'$')
            || CHECK!(string, b',')
            || CHECK!(string, b'_')
            || CHECK!(string, b'.')
            || CHECK!(string, b'~')
            || CHECK!(string, b'*')
            || CHECK!(string, b'\'')
            || CHECK!(string, b'(')
            || CHECK!(string, b')')
            || CHECK!(string, b'[')
            || CHECK!(string, b']')
        {
            WRITE(emitter, &mut string)?;
        } else {
            let mut width = WIDTH!(string);
            loop {
                let prev_width = width;
                width -= 1;
                if !(prev_width != 0) {
                    break;
                }
                let prev_pointer = string.pointer;
                string.pointer = string.pointer.wrapping_offset(1);
                let value = *prev_pointer;
                PUT(emitter, b'%')?;
                PUT(
                    emitter,
                    (value >> 4).force_add(if (value >> 4) < 10 { b'0' } else { b'A' - 10 }),
                )?;
                PUT(
                    emitter,
                    (value & 0x0F).force_add(if (value & 0x0F) < 10 { b'0' } else { b'A' - 10 }),
                )?;
            }
        }
    }
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

unsafe fn yaml_emitter_write_plain_scalar(
    emitter: &mut yaml_emitter_t,
    value: *mut yaml_char_t,
    length: size_t,
    allow_breaks: bool,
) -> Result<(), ()> {
    let mut spaces = false;
    let mut breaks = false;
    let mut string = STRING_ASSIGN!(value, length);
    if !emitter.whitespace && (length != 0 || emitter.flow_level != 0) {
        PUT(emitter, b' ')?;
    }
    while string.pointer != string.end {
        if IS_SPACE!(string) {
            if allow_breaks
                && !spaces
                && emitter.column > emitter.best_width
                && !IS_SPACE_AT!(string, 1)
            {
                yaml_emitter_write_indent(emitter)?;
                MOVE!(string);
            } else {
                WRITE(emitter, &mut string)?;
            }
            spaces = true;
        } else if IS_BREAK!(string) {
            if !breaks && CHECK!(string, b'\n') {
                PUT_BREAK(emitter)?;
            }
            WRITE_BREAK(emitter, &mut string)?;
            emitter.indention = true;
            breaks = true;
        } else {
            if breaks {
                yaml_emitter_write_indent(emitter)?;
            }
            WRITE(emitter, &mut string)?;
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
    value: *mut yaml_char_t,
    length: size_t,
    allow_breaks: bool,
) -> Result<(), ()> {
    let mut spaces = false;
    let mut breaks = false;
    let mut string = STRING_ASSIGN!(value, length);
    yaml_emitter_write_indicator(
        emitter,
        b"'\0" as *const u8 as *const libc::c_char,
        true,
        false,
        false,
    )?;
    while string.pointer != string.end {
        if IS_SPACE!(string) {
            if allow_breaks
                && !spaces
                && emitter.column > emitter.best_width
                && string.pointer != string.start
                && string.pointer != string.end.wrapping_offset(-1_isize)
                && !IS_SPACE_AT!(string, 1)
            {
                yaml_emitter_write_indent(emitter)?;
                MOVE!(string);
            } else {
                WRITE(emitter, &mut string)?;
            }
            spaces = true;
        } else if IS_BREAK!(string) {
            if !breaks && CHECK!(string, b'\n') {
                PUT_BREAK(emitter)?;
            }
            WRITE_BREAK(emitter, &mut string)?;
            emitter.indention = true;
            breaks = true;
        } else {
            if breaks {
                yaml_emitter_write_indent(emitter)?;
            }
            if CHECK!(string, b'\'') {
                PUT(emitter, b'\'')?;
            }
            WRITE(emitter, &mut string)?;
            emitter.indention = false;
            spaces = false;
            breaks = false;
        }
    }
    if breaks {
        yaml_emitter_write_indent(emitter)?;
    }
    yaml_emitter_write_indicator(
        emitter,
        b"'\0" as *const u8 as *const libc::c_char,
        false,
        false,
        false,
    )?;
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

unsafe fn yaml_emitter_write_double_quoted_scalar(
    emitter: &mut yaml_emitter_t,
    value: *mut yaml_char_t,
    length: size_t,
    allow_breaks: bool,
) -> Result<(), ()> {
    let mut spaces = false;
    let mut string = STRING_ASSIGN!(value, length);
    yaml_emitter_write_indicator(
        emitter,
        b"\"\0" as *const u8 as *const libc::c_char,
        true,
        false,
        false,
    )?;
    while string.pointer != string.end {
        if !IS_PRINTABLE!(string)
            || !emitter.unicode && !IS_ASCII!(string)
            || IS_BOM!(string)
            || IS_BREAK!(string)
            || CHECK!(string, b'"')
            || CHECK!(string, b'\\')
        {
            let mut octet: libc::c_uchar;
            let mut width: libc::c_uint;
            let mut value_0: libc::c_uint;
            let mut k: libc::c_int;
            octet = *string.pointer;
            width = if octet & 0x80 == 0x00 {
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
            value_0 = if octet & 0x80 == 0 {
                octet & 0x7F
            } else if octet & 0xE0 == 0xC0 {
                octet & 0x1F
            } else if octet & 0xF0 == 0xE0 {
                octet & 0x0F
            } else if octet & 0xF8 == 0xF0 {
                octet & 0x07
            } else {
                0
            } as libc::c_uint;
            k = 1;
            while k < width as libc::c_int {
                octet = *string.pointer.wrapping_offset(k as isize);
                value_0 = (value_0 << 6).force_add((octet & 0x3F) as libc::c_uint);
                k += 1;
            }
            string.pointer = string.pointer.wrapping_offset(width as isize);
            PUT(emitter, b'\\')?;
            match value_0 {
                0x00 => {
                    PUT(emitter, b'0')?;
                }
                0x07 => {
                    PUT(emitter, b'a')?;
                }
                0x08 => {
                    PUT(emitter, b'b')?;
                }
                0x09 => {
                    PUT(emitter, b't')?;
                }
                0x0A => {
                    PUT(emitter, b'n')?;
                }
                0x0B => {
                    PUT(emitter, b'v')?;
                }
                0x0C => {
                    PUT(emitter, b'f')?;
                }
                0x0D => {
                    PUT(emitter, b'r')?;
                }
                0x1B => {
                    PUT(emitter, b'e')?;
                }
                0x22 => {
                    PUT(emitter, b'"')?;
                }
                0x5C => {
                    PUT(emitter, b'\\')?;
                }
                0x85 => {
                    PUT(emitter, b'N')?;
                }
                0xA0 => {
                    PUT(emitter, b'_')?;
                }
                0x2028 => {
                    PUT(emitter, b'L')?;
                }
                0x2029 => {
                    PUT(emitter, b'P')?;
                }
                _ => {
                    if value_0 <= 0xFF {
                        PUT(emitter, b'x')?;
                        width = 2;
                    } else if value_0 <= 0xFFFF {
                        PUT(emitter, b'u')?;
                        width = 4;
                    } else {
                        PUT(emitter, b'U')?;
                        width = 8;
                    }
                    k = width.wrapping_sub(1).wrapping_mul(4) as libc::c_int;
                    while k >= 0 {
                        let digit: libc::c_int = (value_0 >> k & 0x0F) as libc::c_int;
                        PUT(
                            emitter,
                            (digit + if digit < 10 { b'0' } else { b'A' - 10 } as i32) as u8,
                        )?;
                        k -= 4;
                    }
                }
            }
            spaces = false;
        } else if IS_SPACE!(string) {
            if allow_breaks
                && !spaces
                && emitter.column > emitter.best_width
                && string.pointer != string.start
                && string.pointer != string.end.wrapping_offset(-1_isize)
            {
                yaml_emitter_write_indent(emitter)?;
                if IS_SPACE_AT!(string, 1) {
                    PUT(emitter, b'\\')?;
                }
                MOVE!(string);
            } else {
                WRITE(emitter, &mut string)?;
            }
            spaces = true;
        } else {
            WRITE(emitter, &mut string)?;
            spaces = false;
        }
    }
    yaml_emitter_write_indicator(
        emitter,
        b"\"\0" as *const u8 as *const libc::c_char,
        false,
        false,
        false,
    )?;
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

unsafe fn yaml_emitter_write_block_scalar_hints(
    emitter: &mut yaml_emitter_t,
    string: &yaml_string_t,
) -> Result<(), ()> {
    let mut indent_hint: [libc::c_char; 2] = [0; 2];
    let mut chomp_hint: *const libc::c_char = ptr::null::<libc::c_char>();
    if IS_SPACE!(string) || IS_BREAK!(string) {
        indent_hint[0] = (b'0' as libc::c_int + emitter.best_indent) as libc::c_char;
        indent_hint[1] = '\0' as libc::c_char;
        yaml_emitter_write_indicator(emitter, indent_hint.as_mut_ptr(), false, false, false)?;
    }
    emitter.open_ended = 0;
    let mut pointer = string.end;

    if string.start == pointer {
        chomp_hint = b"-\0" as *const u8 as *const libc::c_char;
    } else {
        loop {
            pointer = pointer.wrapping_offset(-1);
            if !(*pointer & 0xC0 == 0x80) {
                break;
            }
        }
        if !IS_BREAK_PTR!(pointer) {
            chomp_hint = b"-\0" as *const u8 as *const libc::c_char;
        } else if string.start == pointer {
            chomp_hint = b"+\0" as *const u8 as *const libc::c_char;
            emitter.open_ended = 2;
        } else {
            loop {
                pointer = pointer.wrapping_offset(-1);
                if !(*pointer & 0xC0 == 0x80) {
                    break;
                }
            }
            if IS_BREAK_PTR!(pointer) {
                chomp_hint = b"+\0" as *const u8 as *const libc::c_char;
                emitter.open_ended = 2;
            }
        }
    }
    if !chomp_hint.is_null() {
        yaml_emitter_write_indicator(emitter, chomp_hint, false, false, false)?;
    }
    Ok(())
}

unsafe fn yaml_emitter_write_literal_scalar(
    emitter: &mut yaml_emitter_t,
    value: *mut yaml_char_t,
    length: size_t,
) -> Result<(), ()> {
    let mut breaks = true;
    let mut string = STRING_ASSIGN!(value, length);
    yaml_emitter_write_indicator(
        emitter,
        b"|\0" as *const u8 as *const libc::c_char,
        true,
        false,
        false,
    )?;
    yaml_emitter_write_block_scalar_hints(emitter, &string)?;
    PUT_BREAK(emitter)?;
    emitter.indention = true;
    emitter.whitespace = true;
    while string.pointer != string.end {
        if IS_BREAK!(string) {
            WRITE_BREAK(emitter, &mut string)?;
            emitter.indention = true;
            breaks = true;
        } else {
            if breaks {
                yaml_emitter_write_indent(emitter)?;
            }
            WRITE(emitter, &mut string)?;
            emitter.indention = false;
            breaks = false;
        }
    }
    Ok(())
}

unsafe fn yaml_emitter_write_folded_scalar(
    emitter: &mut yaml_emitter_t,
    value: *mut yaml_char_t,
    length: size_t,
) -> Result<(), ()> {
    let mut breaks = true;
    let mut leading_spaces = true;
    let mut string = STRING_ASSIGN!(value, length);
    yaml_emitter_write_indicator(
        emitter,
        b">\0" as *const u8 as *const libc::c_char,
        true,
        false,
        false,
    )?;
    yaml_emitter_write_block_scalar_hints(emitter, &string)?;
    PUT_BREAK(emitter)?;
    emitter.indention = true;
    emitter.whitespace = true;
    while string.pointer != string.end {
        if IS_BREAK!(string) {
            if !breaks && !leading_spaces && CHECK!(string, b'\n') {
                let mut k: libc::c_int = 0;
                while IS_BREAK_AT!(string, k as isize) {
                    k += WIDTH_AT!(string, k as isize);
                }
                if !IS_BLANKZ_AT!(string, k) {
                    PUT_BREAK(emitter)?;
                }
            }
            WRITE_BREAK(emitter, &mut string)?;
            emitter.indention = true;
            breaks = true;
        } else {
            if breaks {
                yaml_emitter_write_indent(emitter)?;
                leading_spaces = IS_BLANK!(string);
            }
            if !breaks
                && IS_SPACE!(string)
                && !IS_SPACE_AT!(string, 1)
                && emitter.column > emitter.best_width
            {
                yaml_emitter_write_indent(emitter)?;
                MOVE!(string);
            } else {
                WRITE(emitter, &mut string)?;
            }
            emitter.indention = false;
            breaks = false;
        }
    }
    Ok(())
}
