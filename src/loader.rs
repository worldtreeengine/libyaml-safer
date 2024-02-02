use alloc::string::String;
use alloc::{vec, vec::Vec};

use crate::yaml::{YamlEventData, YamlNodeData};
use crate::{
    yaml_alias_data_t, yaml_document_new, yaml_document_t, yaml_event_t, yaml_mark_t,
    yaml_node_pair_t, yaml_node_t, yaml_parser_parse, yaml_parser_t, ComposerError,
};

/// Parse the input stream and produce the next YAML document.
///
/// Call this function subsequently to produce a sequence of documents
/// constituting the input stream.
///
/// If the produced document has no root node, it means that the document end
/// has been reached.
///
/// An application must not alternate the calls of yaml_parser_load() with the
/// calls of yaml_parser_scan() or yaml_parser_parse(). Doing this will break
/// the parser.
pub fn yaml_parser_load(parser: &mut yaml_parser_t) -> Result<yaml_document_t, ComposerError> {
    let mut document = yaml_document_new(None, &[], false, false);
    document.nodes.reserve(16);

    if !parser.stream_start_produced {
        match yaml_parser_parse(parser) {
            Ok(yaml_event_t {
                data: YamlEventData::StreamStart { .. },
                ..
            }) => (),
            Ok(_) => panic!("expected stream start"),
            Err(err) => {
                yaml_parser_delete_aliases(parser);
                return Err(err.into());
            }
        }
    }
    if parser.stream_end_produced {
        return Ok(document);
    }
    let err: ComposerError;
    match yaml_parser_parse(parser) {
        Ok(event) => {
            if let YamlEventData::StreamEnd = &event.data {
                return Ok(document);
            }
            parser.aliases.reserve(16);
            match yaml_parser_load_document(parser, event, &mut document) {
                Ok(()) => {
                    yaml_parser_delete_aliases(parser);
                    return Ok(document);
                }
                Err(e) => err = e,
            }
        }
        Err(e) => err = e.into(),
    }
    yaml_parser_delete_aliases(parser);
    Err(err)
}

fn yaml_parser_set_composer_error<T>(
    problem: &'static str,
    problem_mark: yaml_mark_t,
) -> Result<T, ComposerError> {
    Err(ComposerError::Problem {
        problem,
        mark: problem_mark,
    })
}

fn yaml_parser_set_composer_error_context<T>(
    context: &'static str,
    context_mark: yaml_mark_t,
    problem: &'static str,
    problem_mark: yaml_mark_t,
) -> Result<T, ComposerError> {
    Err(ComposerError::ProblemWithContext {
        context,
        context_mark,
        problem,
        mark: problem_mark,
    })
}

fn yaml_parser_delete_aliases(parser: &mut yaml_parser_t) {
    parser.aliases.clear();
}

fn yaml_parser_load_document(
    parser: &mut yaml_parser_t,
    event: yaml_event_t,
    document: &mut yaml_document_t,
) -> Result<(), ComposerError> {
    let mut ctx = vec![];
    if let YamlEventData::DocumentStart {
        version_directive,
        tag_directives,
        implicit,
    } = event.data
    {
        document.version_directive = version_directive;
        document.tag_directives = tag_directives;
        document.start_implicit = implicit;
        document.start_mark = event.start_mark;
        ctx.reserve(16);
        if let Err(err) = yaml_parser_load_nodes(parser, document, &mut ctx) {
            ctx.clear();
            return Err(err);
        }
        ctx.clear();
        Ok(())
    } else {
        panic!("Expected YAML_DOCUMENT_START_EVENT")
    }
}

fn yaml_parser_load_nodes(
    parser: &mut yaml_parser_t,
    document: &mut yaml_document_t,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    let end_implicit;
    let end_mark;

    loop {
        let event = yaml_parser_parse(parser)?;
        match event.data {
            YamlEventData::NoEvent => panic!("empty event"),
            YamlEventData::StreamStart { .. } => panic!("unexpected stream start event"),
            YamlEventData::StreamEnd => panic!("unexpected stream end event"),
            YamlEventData::DocumentStart { .. } => panic!("unexpected document start event"),
            YamlEventData::DocumentEnd { implicit } => {
                end_implicit = implicit;
                end_mark = event.end_mark;
                break;
            }
            YamlEventData::Alias { .. } => {
                yaml_parser_load_alias(parser, event, document, ctx)?;
            }
            YamlEventData::Scalar { .. } => {
                yaml_parser_load_scalar(parser, event, document, ctx)?;
            }
            YamlEventData::SequenceStart { .. } => {
                yaml_parser_load_sequence(parser, event, document, ctx)?;
            }
            YamlEventData::SequenceEnd => {
                yaml_parser_load_sequence_end(parser, event, document, ctx)?;
            }
            YamlEventData::MappingStart { .. } => {
                yaml_parser_load_mapping(parser, event, document, ctx)?;
            }
            YamlEventData::MappingEnd => {
                yaml_parser_load_mapping_end(parser, event, document, ctx)?;
            }
        }
    }
    document.end_implicit = end_implicit;
    document.end_mark = end_mark;
    Ok(())
}

fn yaml_parser_register_anchor(
    parser: &mut yaml_parser_t,
    document: &mut yaml_document_t,
    index: i32,
    anchor: Option<String>,
) -> Result<(), ComposerError> {
    let Some(anchor) = anchor else {
        return Ok(());
    };
    let data = yaml_alias_data_t {
        anchor,
        index,
        mark: document.nodes[index as usize - 1].start_mark,
    };
    for alias_data in &parser.aliases {
        if alias_data.anchor == data.anchor {
            return yaml_parser_set_composer_error_context(
                "found duplicate anchor; first occurrence",
                alias_data.mark,
                "second occurrence",
                data.mark,
            );
        }
    }
    parser.aliases.push(data);
    Ok(())
}

fn yaml_parser_load_node_add(
    document: &mut yaml_document_t,
    ctx: &mut Vec<i32>,
    index: i32,
) -> Result<(), ComposerError> {
    if ctx.is_empty() {
        return Ok(());
    }
    let parent_index: i32 = *ctx.last().unwrap();
    let parent = &mut document.nodes[parent_index as usize - 1];
    match parent.data {
        YamlNodeData::Sequence { ref mut items, .. } => {
            items.push(index);
        }
        YamlNodeData::Mapping { ref mut pairs, .. } => {
            let mut pair = yaml_node_pair_t::default();
            let mut do_push = true;
            if !pairs.is_empty() {
                let p: &mut yaml_node_pair_t = pairs.last_mut().unwrap();
                if p.key != 0 && p.value == 0 {
                    p.value = index;
                    do_push = false;
                }
            }
            if do_push {
                pair.key = index;
                pair.value = 0;
                pairs.push(pair);
            }
        }
        _ => {
            panic!("document parent node is not a sequence or a mapping")
        }
    }
    Ok(())
}

fn yaml_parser_load_alias(
    parser: &mut yaml_parser_t,
    event: yaml_event_t,
    document: &mut yaml_document_t,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    let anchor: &str = if let YamlEventData::Alias { anchor } = &event.data {
        anchor
    } else {
        unreachable!()
    };

    for alias_data in &parser.aliases {
        if alias_data.anchor == anchor {
            return yaml_parser_load_node_add(document, ctx, alias_data.index);
        }
    }

    yaml_parser_set_composer_error("found undefined alias", event.start_mark)
}

fn yaml_parser_load_scalar(
    parser: &mut yaml_parser_t,
    event: yaml_event_t,
    document: &mut yaml_document_t,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    let YamlEventData::Scalar {
        mut tag,
        value,
        style,
        anchor,
        ..
    } = event.data
    else {
        unreachable!()
    };

    if tag.is_none() || tag.as_deref() == Some("!") {
        tag = Some(String::from("tag:yaml.org,2002:str"));
    }
    let node = yaml_node_t {
        data: YamlNodeData::Scalar { value, style },
        tag,
        start_mark: event.start_mark,
        end_mark: event.end_mark,
    };
    document.nodes.push(node);
    let index: i32 = document.nodes.len() as i32;
    yaml_parser_register_anchor(parser, document, index, anchor)?;
    yaml_parser_load_node_add(document, ctx, index)
}

fn yaml_parser_load_sequence(
    parser: &mut yaml_parser_t,
    event: yaml_event_t,
    document: &mut yaml_document_t,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    let YamlEventData::SequenceStart {
        anchor,
        mut tag,
        style,
        ..
    } = event.data
    else {
        unreachable!()
    };

    let mut items = Vec::with_capacity(16);

    if tag.is_none() || tag.as_deref() == Some("!") {
        tag = Some(String::from("tag:yaml.org,2002:seq"));
    }

    let node = yaml_node_t {
        data: YamlNodeData::Sequence {
            items: core::mem::take(&mut items),
            style,
        },
        tag,
        start_mark: event.start_mark,
        end_mark: event.end_mark,
    };

    document.nodes.push(node);
    let index: i32 = document.nodes.len() as i32;
    yaml_parser_register_anchor(parser, document, index, anchor.clone())?;
    yaml_parser_load_node_add(document, ctx, index)?;
    ctx.push(index);
    Ok(())
}

fn yaml_parser_load_sequence_end(
    _parser: &mut yaml_parser_t,
    event: yaml_event_t,
    document: &mut yaml_document_t,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    assert!(!ctx.is_empty());
    let index: i32 = *ctx.last().unwrap();
    assert!(matches!(
        document.nodes[index as usize - 1].data,
        YamlNodeData::Sequence { .. }
    ));
    document.nodes[index as usize - 1].end_mark = event.end_mark;
    _ = ctx.pop();
    Ok(())
}

fn yaml_parser_load_mapping(
    parser: &mut yaml_parser_t,
    event: yaml_event_t,
    document: &mut yaml_document_t,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    let YamlEventData::MappingStart {
        anchor,
        mut tag,
        style,
        ..
    } = event.data
    else {
        unreachable!()
    };

    let mut pairs = Vec::with_capacity(16);

    if tag.is_none() || tag.as_deref() == Some("!") {
        tag = Some(String::from("tag:yaml.org,2002:map"));
    }
    let node = yaml_node_t {
        data: YamlNodeData::Mapping {
            pairs: core::mem::take(&mut pairs),
            style,
        },
        tag,
        start_mark: event.start_mark,
        end_mark: event.end_mark,
    };
    document.nodes.push(node);
    let index: i32 = document.nodes.len() as i32;
    yaml_parser_register_anchor(parser, document, index, anchor)?;
    yaml_parser_load_node_add(document, ctx, index)?;
    ctx.push(index);
    Ok(())
}

fn yaml_parser_load_mapping_end(
    _parser: &mut yaml_parser_t,
    event: yaml_event_t,
    document: &mut yaml_document_t,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    assert!(!ctx.is_empty());
    let index: i32 = *ctx.last().unwrap();
    assert!(matches!(
        document.nodes[index as usize - 1].data,
        YamlNodeData::Mapping { .. }
    ));
    document.nodes[index as usize - 1].end_mark = event.end_mark;
    _ = ctx.pop();
    Ok(())
}
