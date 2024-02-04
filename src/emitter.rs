use alloc::string::String;

use crate::api::OUTPUT_BUFFER_SIZE;
use crate::macros::{
    is_alpha, is_ascii, is_blank, is_blankz, is_bom, is_break, is_breakz, is_printable, is_space,
};
use crate::yaml::EventData;
use crate::{
    yaml_emitter_flush, Break, Emitter, EmitterError, EmitterState, Encoding, Event, MappingStyle,
    ScalarStyle, SequenceStyle, TagDirective, VersionDirective, WriterError,
};

fn FLUSH(emitter: &mut Emitter) -> Result<(), WriterError> {
    if emitter.buffer.len() < OUTPUT_BUFFER_SIZE - 5 {
        Ok(())
    } else {
        yaml_emitter_flush(emitter)
    }
}

fn PUT(emitter: &mut Emitter, value: u8) -> Result<(), WriterError> {
    FLUSH(emitter)?;
    let ch = char::from(value);
    emitter.buffer.push(ch);
    emitter.column += 1;
    Ok(())
}

fn PUT_BREAK(emitter: &mut Emitter) -> Result<(), WriterError> {
    FLUSH(emitter)?;
    if emitter.line_break == Break::Cr {
        emitter.buffer.push('\r');
    } else if emitter.line_break == Break::Ln {
        emitter.buffer.push('\n');
    } else if emitter.line_break == Break::CrLn {
        emitter.buffer.push_str("\r\n");
    };
    emitter.column = 0;
    emitter.line += 1;
    Ok(())
}

/// Write UTF-8 charanters from `string` to `emitter` and increment
/// `emitter.column` the appropriate number of times.
fn WRITE_STR(emitter: &mut Emitter, string: &str) -> Result<(), WriterError> {
    for ch in string.chars() {
        WRITE_CHAR(emitter, ch)?;
    }
    Ok(())
}

fn WRITE_CHAR(emitter: &mut Emitter, ch: char) -> Result<(), WriterError> {
    FLUSH(emitter)?;
    emitter.buffer.push(ch);
    emitter.column += 1;
    Ok(())
}

fn WRITE_BREAK_CHAR(emitter: &mut Emitter, ch: char) -> Result<(), WriterError> {
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

#[derive(Default)]
struct Analysis<'a> {
    pub anchor: Option<AnchorAnalysis<'a>>,
    pub tag: Option<TagAnalysis<'a>>,
    pub scalar: Option<ScalarAnalysis<'a>>,
}

struct AnchorAnalysis<'a> {
    pub anchor: &'a str,
    pub alias: bool,
}

struct TagAnalysis<'a> {
    pub handle: &'a str,
    pub suffix: &'a str,
}

struct ScalarAnalysis<'a> {
    /// The scalar value.
    pub value: &'a str,
    /// Does the scalar contain line breaks?
    pub multiline: bool,
    /// Can the scalar be expessed in the flow plain style?
    pub flow_plain_allowed: bool,
    /// Can the scalar be expressed in the block plain style?
    pub block_plain_allowed: bool,
    /// Can the scalar be expressed in the single quoted style?
    pub single_quoted_allowed: bool,
    /// Can the scalar be expressed in the literal or folded styles?
    pub block_allowed: bool,
    /// The output style.
    pub style: ScalarStyle,
}

fn yaml_emitter_set_emitter_error<T>(
    _emitter: &mut Emitter,
    problem: &'static str,
) -> Result<T, EmitterError> {
    Err(EmitterError::Problem(problem))
}

/// Emit an event.
///
/// The event object may be generated using the
/// [`yaml_parser_parse()`](crate::yaml_parser_parse) function. The emitter
/// takes the responsibility for the event object and destroys its content after
/// it is emitted. The event object is destroyed even if the function fails.
pub fn yaml_emitter_emit(emitter: &mut Emitter, event: Event) -> Result<(), EmitterError> {
    emitter.events.push_back(event);
    while let Some(event) = yaml_emitter_needs_mode_events(emitter) {
        let tag_directives = core::mem::take(&mut emitter.tag_directives);

        let mut analysis = yaml_emitter_analyze_event(emitter, &event, &tag_directives)?;
        yaml_emitter_state_machine(emitter, &event, &mut analysis)?;

        // The DOCUMENT-START event populates the tag directives, and this
        // happens only once, so don't swap out the tags in that case.
        if emitter.tag_directives.is_empty() {
            emitter.tag_directives = tag_directives;
        }
    }
    Ok(())
}

fn yaml_emitter_needs_mode_events(emitter: &mut Emitter) -> Option<Event> {
    let first = emitter.events.front()?;

    let accummulate = match &first.data {
        EventData::DocumentStart { .. } => 1,
        EventData::SequenceStart { .. } => 2,
        EventData::MappingStart { .. } => 3,
        _ => return emitter.events.pop_front(),
    };

    if emitter.events.len() > accummulate {
        return emitter.events.pop_front();
    }

    let mut level = 0;
    for event in &emitter.events {
        match event.data {
            EventData::StreamStart { .. }
            | EventData::DocumentStart { .. }
            | EventData::SequenceStart { .. }
            | EventData::MappingStart { .. } => {
                level += 1;
            }

            EventData::StreamEnd
            | EventData::DocumentEnd { .. }
            | EventData::SequenceEnd
            | EventData::MappingEnd => {
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

fn yaml_emitter_append_tag_directive(
    emitter: &mut Emitter,
    value: TagDirective,
    allow_duplicates: bool,
) -> Result<(), EmitterError> {
    for tag_directive in &emitter.tag_directives {
        if value.handle == tag_directive.handle {
            if allow_duplicates {
                return Ok(());
            }
            return yaml_emitter_set_emitter_error(emitter, "duplicate %TAG directive");
        }
    }
    emitter.tag_directives.push(value);
    Ok(())
}

fn yaml_emitter_increase_indent(emitter: &mut Emitter, flow: bool, indentless: bool) {
    emitter.indents.push(emitter.indent);
    if emitter.indent < 0 {
        emitter.indent = if flow { emitter.best_indent } else { 0 };
    } else if !indentless {
        emitter.indent += emitter.best_indent;
    }
}

fn yaml_emitter_state_machine<'a>(
    emitter: &mut Emitter,
    event: &'a Event,
    analysis: &mut Analysis<'a>,
) -> Result<(), EmitterError> {
    match emitter.state {
        EmitterState::StreamStart => yaml_emitter_emit_stream_start(emitter, event),
        EmitterState::FirstDocumentStart => yaml_emitter_emit_document_start(emitter, event, true),
        EmitterState::DocumentStart => yaml_emitter_emit_document_start(emitter, event, false),
        EmitterState::DocumentContent => {
            yaml_emitter_emit_document_content(emitter, event, analysis)
        }
        EmitterState::DocumentEnd => yaml_emitter_emit_document_end(emitter, event),
        EmitterState::FlowSequenceFirstItem => {
            yaml_emitter_emit_flow_sequence_item(emitter, event, true, analysis)
        }
        EmitterState::FlowSequenceItem => {
            yaml_emitter_emit_flow_sequence_item(emitter, event, false, analysis)
        }
        EmitterState::FlowMappingFirstKey => {
            yaml_emitter_emit_flow_mapping_key(emitter, event, true, analysis)
        }
        EmitterState::FlowMappingKey => {
            yaml_emitter_emit_flow_mapping_key(emitter, event, false, analysis)
        }
        EmitterState::FlowMappingSimpleValue => {
            yaml_emitter_emit_flow_mapping_value(emitter, event, true, analysis)
        }
        EmitterState::FlowMappingValue => {
            yaml_emitter_emit_flow_mapping_value(emitter, event, false, analysis)
        }
        EmitterState::BlockSequenceFirstItem => {
            yaml_emitter_emit_block_sequence_item(emitter, event, true, analysis)
        }
        EmitterState::BlockSequenceItem => {
            yaml_emitter_emit_block_sequence_item(emitter, event, false, analysis)
        }
        EmitterState::BlockMappingFirstKey => {
            yaml_emitter_emit_block_mapping_key(emitter, event, true, analysis)
        }
        EmitterState::BlockMappingKey => {
            yaml_emitter_emit_block_mapping_key(emitter, event, false, analysis)
        }
        EmitterState::BlockMappingSimpleValue => {
            yaml_emitter_emit_block_mapping_value(emitter, event, true, analysis)
        }
        EmitterState::BlockMappingValue => {
            yaml_emitter_emit_block_mapping_value(emitter, event, false, analysis)
        }
        EmitterState::End => {
            yaml_emitter_set_emitter_error(emitter, "expected nothing after STREAM-END")
        }
    }
}

fn yaml_emitter_emit_stream_start(
    emitter: &mut Emitter,
    event: &Event,
) -> Result<(), EmitterError> {
    emitter.open_ended = 0;
    if let EventData::StreamStart { ref encoding } = event.data {
        if emitter.encoding == Encoding::Any {
            emitter.encoding = *encoding;
        }
        if emitter.encoding == Encoding::Any {
            emitter.encoding = Encoding::Utf8;
        }
        if emitter.best_indent < 2 || emitter.best_indent > 9 {
            emitter.best_indent = 2;
        }
        if emitter.best_width >= 0 && emitter.best_width <= emitter.best_indent * 2 {
            emitter.best_width = 80;
        }
        if emitter.best_width < 0 {
            emitter.best_width = i32::MAX;
        }
        if emitter.line_break == Break::Any {
            emitter.line_break = Break::Ln;
        }
        emitter.indent = -1;
        emitter.line = 0;
        emitter.column = 0;
        emitter.whitespace = true;
        emitter.indention = true;
        if emitter.encoding != Encoding::Utf8 {
            yaml_emitter_write_bom(emitter)?;
        }
        emitter.state = EmitterState::FirstDocumentStart;
        return Ok(());
    }
    yaml_emitter_set_emitter_error(emitter, "expected STREAM-START")
}

fn yaml_emitter_emit_document_start(
    emitter: &mut Emitter,
    event: &Event,
    first: bool,
) -> Result<(), EmitterError> {
    if let EventData::DocumentStart {
        version_directive,
        tag_directives,
        implicit,
    } = &event.data
    {
        let default_tag_directives: [TagDirective; 2] = [
            // TODO: Avoid these heap allocations.
            TagDirective {
                handle: String::from("!"),
                prefix: String::from("!"),
            },
            TagDirective {
                handle: String::from("!!"),
                prefix: String::from("tag:yaml.org,2002:"),
            },
        ];
        let mut implicit = *implicit;
        if let Some(version_directive) = version_directive {
            yaml_emitter_analyze_version_directive(emitter, *version_directive)?;
        }
        for tag_directive in tag_directives {
            yaml_emitter_analyze_tag_directive(emitter, tag_directive)?;
            yaml_emitter_append_tag_directive(emitter, tag_directive.clone(), false)?;
        }
        for tag_directive in default_tag_directives {
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
            for tag_directive in tag_directives {
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
        emitter.state = EmitterState::DocumentContent;
        emitter.open_ended = 0;
        return Ok(());
    } else if let EventData::StreamEnd = &event.data {
        if emitter.open_ended == 2 {
            yaml_emitter_write_indicator(emitter, "...", true, false, false)?;
            emitter.open_ended = 0;
            yaml_emitter_write_indent(emitter)?;
        }
        yaml_emitter_flush(emitter)?;
        emitter.state = EmitterState::End;
        return Ok(());
    }

    yaml_emitter_set_emitter_error(emitter, "expected DOCUMENT-START or STREAM-END")
}

fn yaml_emitter_emit_document_content(
    emitter: &mut Emitter,
    event: &Event,
    analysis: &mut Analysis,
) -> Result<(), EmitterError> {
    emitter.states.push(EmitterState::DocumentEnd);
    yaml_emitter_emit_node(emitter, event, true, false, false, false, analysis)
}

fn yaml_emitter_emit_document_end(
    emitter: &mut Emitter,
    event: &Event,
) -> Result<(), EmitterError> {
    if let EventData::DocumentEnd { implicit } = &event.data {
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
        emitter.state = EmitterState::DocumentStart;
        emitter.tag_directives.clear();
        return Ok(());
    }

    yaml_emitter_set_emitter_error(emitter, "expected DOCUMENT-END")
}

fn yaml_emitter_emit_flow_sequence_item(
    emitter: &mut Emitter,
    event: &Event,
    first: bool,
    analysis: &mut Analysis,
) -> Result<(), EmitterError> {
    if first {
        yaml_emitter_write_indicator(emitter, "[", true, true, false)?;
        yaml_emitter_increase_indent(emitter, true, false);
        emitter.flow_level += 1;
    }
    if let EventData::SequenceEnd = &event.data {
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
    emitter.states.push(EmitterState::FlowSequenceItem);
    yaml_emitter_emit_node(emitter, event, false, true, false, false, analysis)
}

fn yaml_emitter_emit_flow_mapping_key(
    emitter: &mut Emitter,
    event: &Event,
    first: bool,
    analysis: &mut Analysis,
) -> Result<(), EmitterError> {
    if first {
        yaml_emitter_write_indicator(emitter, "{", true, true, false)?;
        yaml_emitter_increase_indent(emitter, true, false);
        emitter.flow_level += 1;
    }
    if let EventData::MappingEnd = &event.data {
        assert!(
            !emitter.indents.is_empty(),
            "emitter.indents should not be empty"
        );
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
    if !emitter.canonical && yaml_emitter_check_simple_key(emitter, event, analysis) {
        emitter.states.push(EmitterState::FlowMappingSimpleValue);
        yaml_emitter_emit_node(emitter, event, false, false, true, true, analysis)
    } else {
        yaml_emitter_write_indicator(emitter, "?", true, false, false)?;
        emitter.states.push(EmitterState::FlowMappingValue);
        yaml_emitter_emit_node(emitter, event, false, false, true, false, analysis)
    }
}

fn yaml_emitter_emit_flow_mapping_value(
    emitter: &mut Emitter,
    event: &Event,
    simple: bool,
    analysis: &mut Analysis,
) -> Result<(), EmitterError> {
    if simple {
        yaml_emitter_write_indicator(emitter, ":", false, false, false)?;
    } else {
        if emitter.canonical || emitter.column > emitter.best_width {
            yaml_emitter_write_indent(emitter)?;
        }
        yaml_emitter_write_indicator(emitter, ":", true, false, false)?;
    }
    emitter.states.push(EmitterState::FlowMappingKey);
    yaml_emitter_emit_node(emitter, event, false, false, true, false, analysis)
}

fn yaml_emitter_emit_block_sequence_item(
    emitter: &mut Emitter,
    event: &Event,
    first: bool,
    analysis: &mut Analysis,
) -> Result<(), EmitterError> {
    if first {
        yaml_emitter_increase_indent(
            emitter,
            false,
            emitter.mapping_context && !emitter.indention,
        );
    }
    if let EventData::SequenceEnd = &event.data {
        emitter.indent = emitter.indents.pop().unwrap();
        emitter.state = emitter.states.pop().unwrap();
        return Ok(());
    }
    yaml_emitter_write_indent(emitter)?;
    yaml_emitter_write_indicator(emitter, "-", true, false, true)?;
    emitter.states.push(EmitterState::BlockSequenceItem);
    yaml_emitter_emit_node(emitter, event, false, true, false, false, analysis)
}

fn yaml_emitter_emit_block_mapping_key(
    emitter: &mut Emitter,
    event: &Event,
    first: bool,
    analysis: &mut Analysis,
) -> Result<(), EmitterError> {
    if first {
        yaml_emitter_increase_indent(emitter, false, false);
    }
    if let EventData::MappingEnd = &event.data {
        emitter.indent = emitter.indents.pop().unwrap();
        emitter.state = emitter.states.pop().unwrap();
        return Ok(());
    }
    yaml_emitter_write_indent(emitter)?;
    if yaml_emitter_check_simple_key(emitter, event, analysis) {
        emitter.states.push(EmitterState::BlockMappingSimpleValue);
        yaml_emitter_emit_node(emitter, event, false, false, true, true, analysis)
    } else {
        yaml_emitter_write_indicator(emitter, "?", true, false, true)?;
        emitter.states.push(EmitterState::BlockMappingValue);
        yaml_emitter_emit_node(emitter, event, false, false, true, false, analysis)
    }
}

fn yaml_emitter_emit_block_mapping_value(
    emitter: &mut Emitter,
    event: &Event,
    simple: bool,
    analysis: &mut Analysis,
) -> Result<(), EmitterError> {
    if simple {
        yaml_emitter_write_indicator(emitter, ":", false, false, false)?;
    } else {
        yaml_emitter_write_indent(emitter)?;
        yaml_emitter_write_indicator(emitter, ":", true, false, true)?;
    }
    emitter.states.push(EmitterState::BlockMappingKey);
    yaml_emitter_emit_node(emitter, event, false, false, true, false, analysis)
}

fn yaml_emitter_emit_node(
    emitter: &mut Emitter,
    event: &Event,
    root: bool,
    sequence: bool,
    mapping: bool,
    simple_key: bool,
    analysis: &mut Analysis,
) -> Result<(), EmitterError> {
    emitter.root_context = root;
    emitter.sequence_context = sequence;
    emitter.mapping_context = mapping;
    emitter.simple_key_context = simple_key;

    match event.data {
        EventData::Alias { .. } => yaml_emitter_emit_alias(emitter, event, &analysis.anchor),
        EventData::Scalar { .. } => yaml_emitter_emit_scalar(emitter, event, analysis),
        EventData::SequenceStart { .. } => {
            yaml_emitter_emit_sequence_start(emitter, event, analysis)
        }
        EventData::MappingStart { .. } => yaml_emitter_emit_mapping_start(emitter, event, analysis),
        _ => yaml_emitter_set_emitter_error(
            emitter,
            "expected SCALAR, SEQUENCE-START, MAPPING-START, or ALIAS",
        ),
    }
}

fn yaml_emitter_emit_alias(
    emitter: &mut Emitter,
    _event: &Event,
    analysis: &Option<AnchorAnalysis>,
) -> Result<(), EmitterError> {
    yaml_emitter_process_anchor(emitter, analysis)?;
    if emitter.simple_key_context {
        PUT(emitter, b' ')?;
    }
    emitter.state = emitter.states.pop().unwrap();
    Ok(())
}

fn yaml_emitter_emit_scalar(
    emitter: &mut Emitter,
    event: &Event,
    analysis: &mut Analysis,
) -> Result<(), EmitterError> {
    let Analysis {
        anchor,
        tag,
        scalar: Some(scalar),
    } = analysis
    else {
        unreachable!("no scalar analysis");
    };

    yaml_emitter_select_scalar_style(emitter, event, scalar, tag)?;
    yaml_emitter_process_anchor(emitter, anchor)?;
    yaml_emitter_process_tag(emitter, tag)?;
    yaml_emitter_increase_indent(emitter, true, false);
    yaml_emitter_process_scalar(emitter, scalar)?;
    emitter.indent = emitter.indents.pop().unwrap();
    emitter.state = emitter.states.pop().unwrap();
    Ok(())
}

fn yaml_emitter_emit_sequence_start(
    emitter: &mut Emitter,
    event: &Event,
    analysis: &Analysis,
) -> Result<(), EmitterError> {
    let Analysis { anchor, tag, .. } = analysis;
    yaml_emitter_process_anchor(emitter, anchor)?;
    yaml_emitter_process_tag(emitter, tag)?;

    let style = if let EventData::SequenceStart { style, .. } = &event.data {
        *style
    } else {
        unreachable!()
    };

    if emitter.flow_level != 0
        || emitter.canonical
        || style == SequenceStyle::Flow
        || yaml_emitter_check_empty_sequence(emitter, event)
    {
        emitter.state = EmitterState::FlowSequenceFirstItem;
    } else {
        emitter.state = EmitterState::BlockSequenceFirstItem;
    };
    Ok(())
}

fn yaml_emitter_emit_mapping_start(
    emitter: &mut Emitter,
    event: &Event,
    analysis: &Analysis,
) -> Result<(), EmitterError> {
    let Analysis { anchor, tag, .. } = analysis;
    yaml_emitter_process_anchor(emitter, anchor)?;
    yaml_emitter_process_tag(emitter, tag)?;

    let style = if let EventData::MappingStart { style, .. } = &event.data {
        *style
    } else {
        unreachable!()
    };

    if emitter.flow_level != 0
        || emitter.canonical
        || style == MappingStyle::Flow
        || yaml_emitter_check_empty_mapping(emitter, event)
    {
        emitter.state = EmitterState::FlowMappingFirstKey;
    } else {
        emitter.state = EmitterState::BlockMappingFirstKey;
    }
    Ok(())
}

fn yaml_emitter_check_empty_document(_emitter: &Emitter) -> bool {
    false
}

fn yaml_emitter_check_empty_sequence(emitter: &Emitter, event: &Event) -> bool {
    if emitter.events.is_empty() {
        return false;
    }
    let start = matches!(event.data, EventData::SequenceStart { .. });
    let end = matches!(emitter.events[0].data, EventData::SequenceEnd);
    start && end
}

fn yaml_emitter_check_empty_mapping(emitter: &Emitter, event: &Event) -> bool {
    if emitter.events.is_empty() {
        return false;
    }
    let start = matches!(event.data, EventData::MappingStart { .. });
    let end = matches!(emitter.events[0].data, EventData::MappingEnd);
    start && end
}

fn yaml_emitter_check_simple_key(emitter: &Emitter, event: &Event, analysis: &Analysis) -> bool {
    let Analysis {
        tag,
        anchor,
        scalar,
    } = analysis;

    let mut length = anchor.as_ref().map_or(0, |a| a.anchor.len())
        + tag.as_ref().map_or(0, |t| t.handle.len() + t.suffix.len());

    match event.data {
        EventData::Alias { .. } => {
            length = analysis.anchor.as_ref().map_or(0, |a| a.anchor.len());
        }
        EventData::Scalar { .. } => {
            let Some(scalar) = scalar else {
                panic!("no analysis for scalar")
            };

            if scalar.multiline {
                return false;
            }
            length += scalar.value.len();
        }
        EventData::SequenceStart { .. } => {
            if !yaml_emitter_check_empty_sequence(emitter, event) {
                return false;
            }
        }
        EventData::MappingStart { .. } => {
            if !yaml_emitter_check_empty_mapping(emitter, event) {
                return false;
            }
        }
        _ => return false,
    }

    if length > 128 {
        return false;
    }

    true
}

fn yaml_emitter_select_scalar_style(
    emitter: &mut Emitter,
    event: &Event,
    scalar_analysis: &mut ScalarAnalysis,
    tag_analysis: &mut Option<TagAnalysis>,
) -> Result<(), EmitterError> {
    if let EventData::Scalar {
        plain_implicit,
        quoted_implicit,
        style,
        ..
    } = &event.data
    {
        let mut style: ScalarStyle = *style;
        let no_tag = tag_analysis.is_none();
        if no_tag && !*plain_implicit && !*quoted_implicit {
            yaml_emitter_set_emitter_error(
                emitter,
                "neither tag nor implicit flags are specified",
            )?;
        }
        if style == ScalarStyle::Any {
            style = ScalarStyle::Plain;
        }
        if emitter.canonical {
            style = ScalarStyle::DoubleQuoted;
        }
        if emitter.simple_key_context && scalar_analysis.multiline {
            style = ScalarStyle::DoubleQuoted;
        }
        if style == ScalarStyle::Plain {
            if emitter.flow_level != 0 && !scalar_analysis.flow_plain_allowed
                || emitter.flow_level == 0 && !scalar_analysis.block_plain_allowed
            {
                style = ScalarStyle::SingleQuoted;
            }
            if scalar_analysis.value.is_empty()
                && (emitter.flow_level != 0 || emitter.simple_key_context)
            {
                style = ScalarStyle::SingleQuoted;
            }
            if no_tag && !*plain_implicit {
                style = ScalarStyle::SingleQuoted;
            }
        }
        if style == ScalarStyle::SingleQuoted && !scalar_analysis.single_quoted_allowed {
            style = ScalarStyle::DoubleQuoted;
        }
        if (style == ScalarStyle::Literal || style == ScalarStyle::Folded)
            && (!scalar_analysis.block_allowed
                || emitter.flow_level != 0
                || emitter.simple_key_context)
        {
            style = ScalarStyle::DoubleQuoted;
        }
        if no_tag && !*quoted_implicit && style != ScalarStyle::Plain {
            *tag_analysis = Some(TagAnalysis {
                handle: "!",
                suffix: "",
            });
        }
        scalar_analysis.style = style;
        Ok(())
    } else {
        unreachable!()
    }
}

fn yaml_emitter_process_anchor(
    emitter: &mut Emitter,
    analysis: &Option<AnchorAnalysis>,
) -> Result<(), EmitterError> {
    let Some(analysis) = analysis.as_ref() else {
        return Ok(());
    };
    yaml_emitter_write_indicator(
        emitter,
        if analysis.alias { "*" } else { "&" },
        true,
        false,
        false,
    )?;
    yaml_emitter_write_anchor(emitter, analysis.anchor)
}

fn yaml_emitter_process_tag(
    emitter: &mut Emitter,
    analysis: &Option<TagAnalysis>,
) -> Result<(), EmitterError> {
    let Some(analysis) = analysis.as_ref() else {
        return Ok(());
    };

    if analysis.handle.is_empty() && analysis.suffix.is_empty() {
        return Ok(());
    }
    if analysis.handle.is_empty() {
        yaml_emitter_write_indicator(emitter, "!<", true, false, false)?;
        yaml_emitter_write_tag_content(emitter, analysis.suffix, false)?;
        yaml_emitter_write_indicator(emitter, ">", false, false, false)?;
    } else {
        yaml_emitter_write_tag_handle(emitter, analysis.handle)?;
        if !analysis.suffix.is_empty() {
            yaml_emitter_write_tag_content(emitter, analysis.suffix, false)?;
        }
    }
    Ok(())
}

fn yaml_emitter_process_scalar(
    emitter: &mut Emitter,
    analysis: &ScalarAnalysis,
) -> Result<(), EmitterError> {
    match analysis.style {
        ScalarStyle::Plain => {
            yaml_emitter_write_plain_scalar(emitter, analysis.value, !emitter.simple_key_context)
        }
        ScalarStyle::SingleQuoted => yaml_emitter_write_single_quoted_scalar(
            emitter,
            analysis.value,
            !emitter.simple_key_context,
        ),
        ScalarStyle::DoubleQuoted => yaml_emitter_write_double_quoted_scalar(
            emitter,
            analysis.value,
            !emitter.simple_key_context,
        ),
        ScalarStyle::Literal => yaml_emitter_write_literal_scalar(emitter, analysis.value),
        ScalarStyle::Folded => yaml_emitter_write_folded_scalar(emitter, analysis.value),
        ScalarStyle::Any => unreachable!("No scalar style chosen"),
    }
}

fn yaml_emitter_analyze_version_directive(
    emitter: &mut Emitter,
    version_directive: VersionDirective,
) -> Result<(), EmitterError> {
    if version_directive.major != 1 || version_directive.minor != 1 && version_directive.minor != 2
    {
        return yaml_emitter_set_emitter_error(emitter, "incompatible %YAML directive");
    }
    Ok(())
}

fn yaml_emitter_analyze_tag_directive(
    emitter: &mut Emitter,
    tag_directive: &TagDirective,
) -> Result<(), EmitterError> {
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

fn yaml_emitter_analyze_anchor<'a>(
    emitter: &mut Emitter,
    anchor: &'a str,
    alias: bool,
) -> Result<AnchorAnalysis<'a>, EmitterError> {
    if anchor.is_empty() {
        yaml_emitter_set_emitter_error(
            emitter,
            if alias {
                "alias value must not be empty"
            } else {
                "anchor value must not be empty"
            },
        )?;
    }

    for ch in anchor.chars() {
        if !IS_ALPHA_CHAR!(ch) {
            yaml_emitter_set_emitter_error(
                emitter,
                if alias {
                    "alias value must contain alphanumerical characters only"
                } else {
                    "anchor value must contain alphanumerical characters only"
                },
            )?;
        }
    }

    Ok(AnchorAnalysis { anchor, alias })
}

fn yaml_emitter_analyze_tag<'a>(
    emitter: &mut Emitter,
    tag: &'a str,
    tag_directives: &'a [TagDirective],
) -> Result<TagAnalysis<'a>, EmitterError> {
    if tag.is_empty() {
        yaml_emitter_set_emitter_error(emitter, "tag value must not be empty")?;
    }

    let mut handle = "";
    let mut suffix = tag;

    for tag_directive in tag_directives {
        let prefix_len = tag_directive.prefix.len();
        if prefix_len < tag.len() && tag_directive.prefix == tag[0..prefix_len] {
            handle = &tag_directive.handle;
            suffix = &tag[prefix_len..];
            break;
        }
    }

    Ok(TagAnalysis { handle, suffix })
}

fn yaml_emitter_analyze_scalar<'a>(
    emitter: &mut Emitter,
    value: &'a str,
) -> Result<ScalarAnalysis<'a>, EmitterError> {
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

    if value.is_empty() {
        return Ok(ScalarAnalysis {
            value: "",
            multiline: false,
            flow_plain_allowed: false,
            block_plain_allowed: true,
            single_quoted_allowed: true,
            block_allowed: false,
            style: ScalarStyle::Any,
        });
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

    let mut analysis = ScalarAnalysis {
        value,
        multiline: line_breaks,
        flow_plain_allowed: true,
        block_plain_allowed: true,
        single_quoted_allowed: true,
        block_allowed: true,
        style: ScalarStyle::Any,
    };

    analysis.multiline = line_breaks;
    analysis.flow_plain_allowed = true;
    analysis.block_plain_allowed = true;
    analysis.single_quoted_allowed = true;
    analysis.block_allowed = true;
    if leading_space || leading_break || trailing_space || trailing_break {
        analysis.flow_plain_allowed = false;
        analysis.block_plain_allowed = false;
    }
    if trailing_space {
        analysis.block_allowed = false;
    }
    if break_space {
        analysis.flow_plain_allowed = false;
        analysis.block_plain_allowed = false;
        analysis.single_quoted_allowed = false;
    }
    if space_break || special_characters {
        analysis.flow_plain_allowed = false;
        analysis.block_plain_allowed = false;
        analysis.single_quoted_allowed = false;
        analysis.block_allowed = false;
    }
    if line_breaks {
        analysis.flow_plain_allowed = false;
        analysis.block_plain_allowed = false;
    }
    if flow_indicators {
        analysis.flow_plain_allowed = false;
    }
    if block_indicators {
        analysis.block_plain_allowed = false;
    }
    Ok(analysis)
}

fn yaml_emitter_analyze_event<'a>(
    emitter: &mut Emitter,
    event: &'a Event,
    tag_directives: &'a [TagDirective],
) -> Result<Analysis<'a>, EmitterError> {
    let mut analysis = Analysis::default();

    match &event.data {
        EventData::Alias { anchor } => {
            analysis.anchor = Some(yaml_emitter_analyze_anchor(emitter, anchor, true)?);
        }
        EventData::Scalar {
            anchor,
            tag,
            value,
            plain_implicit,
            quoted_implicit,
            ..
        } => {
            let (plain_implicit, quoted_implicit) = (*plain_implicit, *quoted_implicit);
            if let Some(anchor) = anchor {
                analysis.anchor = Some(yaml_emitter_analyze_anchor(emitter, anchor, false)?);
            }
            if tag.is_some() && (emitter.canonical || !plain_implicit && !quoted_implicit) {
                analysis.tag = Some(yaml_emitter_analyze_tag(
                    emitter,
                    tag.as_deref().unwrap(),
                    tag_directives,
                )?);
            }
            analysis.scalar = Some(yaml_emitter_analyze_scalar(emitter, value)?);
        }
        EventData::SequenceStart {
            anchor,
            tag,
            implicit,
            ..
        } => {
            if let Some(anchor) = anchor {
                analysis.anchor = Some(yaml_emitter_analyze_anchor(emitter, anchor, false)?);
            }
            if tag.is_some() && (emitter.canonical || !*implicit) {
                analysis.tag = Some(yaml_emitter_analyze_tag(
                    emitter,
                    tag.as_deref().unwrap(),
                    tag_directives,
                )?);
            }
        }
        EventData::MappingStart {
            anchor,
            tag,
            implicit,
            ..
        } => {
            if let Some(anchor) = anchor {
                analysis.anchor = Some(yaml_emitter_analyze_anchor(emitter, anchor, false)?);
            }
            if tag.is_some() && (emitter.canonical || !*implicit) {
                analysis.tag = Some(yaml_emitter_analyze_tag(
                    emitter,
                    tag.as_deref().unwrap(),
                    tag_directives,
                )?);
            }
        }
        _ => {}
    }

    Ok(analysis)
}

fn yaml_emitter_write_bom(emitter: &mut Emitter) -> Result<(), EmitterError> {
    FLUSH(emitter)?;
    emitter.buffer.push('\u{feff}');
    Ok(())
}

fn yaml_emitter_write_indent(emitter: &mut Emitter) -> Result<(), EmitterError> {
    let indent = if emitter.indent >= 0 {
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

fn yaml_emitter_write_indicator(
    emitter: &mut Emitter,
    indicator: &str,
    need_whitespace: bool,
    is_whitespace: bool,
    is_indention: bool,
) -> Result<(), EmitterError> {
    if need_whitespace && !emitter.whitespace {
        PUT(emitter, b' ')?;
    }
    WRITE_STR(emitter, indicator)?;
    emitter.whitespace = is_whitespace;
    emitter.indention = emitter.indention && is_indention;
    Ok(())
}

fn yaml_emitter_write_anchor(emitter: &mut Emitter, value: &str) -> Result<(), EmitterError> {
    WRITE_STR(emitter, value)?;
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

fn yaml_emitter_write_tag_handle(emitter: &mut Emitter, value: &str) -> Result<(), EmitterError> {
    if !emitter.whitespace {
        PUT(emitter, b' ')?;
    }
    WRITE_STR(emitter, value)?;
    emitter.whitespace = false;
    emitter.indention = false;
    Ok(())
}

fn yaml_emitter_write_tag_content(
    emitter: &mut Emitter,
    value: &str,
    need_whitespace: bool,
) -> Result<(), EmitterError> {
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

fn yaml_emitter_write_plain_scalar(
    emitter: &mut Emitter,
    value: &str,
    allow_breaks: bool,
) -> Result<(), EmitterError> {
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

fn yaml_emitter_write_single_quoted_scalar(
    emitter: &mut Emitter,
    value: &str,
    allow_breaks: bool,
) -> Result<(), EmitterError> {
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

fn yaml_emitter_write_double_quoted_scalar(
    emitter: &mut Emitter,
    value: &str,
    allow_breaks: bool,
) -> Result<(), EmitterError> {
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
                    let mut k = (width - 1) * 4;
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
                && chars.clone().next().is_some()
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

fn yaml_emitter_write_block_scalar_hints(
    emitter: &mut Emitter,
    string: &str,
) -> Result<(), EmitterError> {
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
        } else if is_breakz(next) {
            chomp_hint = Some("+");
            emitter.open_ended = 2;
        }
    }

    if let Some(chomp_hint) = chomp_hint {
        yaml_emitter_write_indicator(emitter, chomp_hint, false, false, false)?;
    }
    Ok(())
}

fn yaml_emitter_write_literal_scalar(
    emitter: &mut Emitter,
    value: &str,
) -> Result<(), EmitterError> {
    let mut breaks = true;
    yaml_emitter_write_indicator(emitter, "|", true, false, false)?;
    yaml_emitter_write_block_scalar_hints(emitter, value)?;
    PUT_BREAK(emitter)?;
    emitter.indention = true;
    emitter.whitespace = true;
    let chars = value.chars();
    for ch in chars {
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

fn yaml_emitter_write_folded_scalar(
    emitter: &mut Emitter,
    value: &str,
) -> Result<(), EmitterError> {
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
