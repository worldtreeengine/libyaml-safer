use alloc::string::String;
use alloc::{vec, vec::Vec};

use crate::{
    yaml_parser_parse, AliasData, ComposerError, Document, Event, EventData, Mark, Node, NodeData,
    NodePair, Parser, DEFAULT_MAPPING_TAG, DEFAULT_SCALAR_TAG, DEFAULT_SEQUENCE_TAG,
};

/// Parse the input stream and produce the next YAML document.
///
/// Call this function subsequently to produce a sequence of documents
/// constituting the input stream.
///
/// If the produced document has no root node, it means that the document end
/// has been reached.
///
/// An application must not alternate the calls of
/// [`yaml_parser_load()`](crate::yaml_parser_load) with the calls of
/// [`yaml_parser_scan()`](crate::yaml_parser_scan) or
/// [`yaml_parser_parse()`](crate::yaml_parser_parse). Doing this will break the
/// parser.
pub fn yaml_parser_load(parser: &mut Parser) -> Result<Document, ComposerError> {
    let mut document = Document::new(None, &[], false, false);
    document.nodes.reserve(16);

    if !parser.stream_start_produced {
        match yaml_parser_parse(parser) {
            Ok(Event {
                data: EventData::StreamStart { .. },
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
            if let EventData::StreamEnd = &event.data {
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
    problem_mark: Mark,
) -> Result<T, ComposerError> {
    Err(ComposerError::Problem {
        problem,
        mark: problem_mark,
    })
}

fn yaml_parser_set_composer_error_context<T>(
    context: &'static str,
    context_mark: Mark,
    problem: &'static str,
    problem_mark: Mark,
) -> Result<T, ComposerError> {
    Err(ComposerError::ProblemWithContext {
        context,
        context_mark,
        problem,
        mark: problem_mark,
    })
}

fn yaml_parser_delete_aliases(parser: &mut Parser) {
    parser.aliases.clear();
}

fn yaml_parser_load_document(
    parser: &mut Parser,
    event: Event,
    document: &mut Document,
) -> Result<(), ComposerError> {
    let mut ctx = vec![];
    if let EventData::DocumentStart {
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
    parser: &mut Parser,
    document: &mut Document,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    let end_implicit;
    let end_mark;

    loop {
        let event = yaml_parser_parse(parser)?;
        match event.data {
            EventData::NoEvent => panic!("empty event"),
            EventData::StreamStart { .. } => panic!("unexpected stream start event"),
            EventData::StreamEnd => panic!("unexpected stream end event"),
            EventData::DocumentStart { .. } => panic!("unexpected document start event"),
            EventData::DocumentEnd { implicit } => {
                end_implicit = implicit;
                end_mark = event.end_mark;
                break;
            }
            EventData::Alias { .. } => {
                yaml_parser_load_alias(parser, event, document, ctx)?;
            }
            EventData::Scalar { .. } => {
                yaml_parser_load_scalar(parser, event, document, ctx)?;
            }
            EventData::SequenceStart { .. } => {
                yaml_parser_load_sequence(parser, event, document, ctx)?;
            }
            EventData::SequenceEnd => {
                yaml_parser_load_sequence_end(parser, event, document, ctx)?;
            }
            EventData::MappingStart { .. } => {
                yaml_parser_load_mapping(parser, event, document, ctx)?;
            }
            EventData::MappingEnd => {
                yaml_parser_load_mapping_end(parser, event, document, ctx)?;
            }
        }
    }
    document.end_implicit = end_implicit;
    document.end_mark = end_mark;
    Ok(())
}

fn yaml_parser_register_anchor(
    parser: &mut Parser,
    document: &mut Document,
    index: i32,
    anchor: Option<String>,
) -> Result<(), ComposerError> {
    let Some(anchor) = anchor else {
        return Ok(());
    };
    let data = AliasData {
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
    document: &mut Document,
    ctx: &[i32],
    index: i32,
) -> Result<(), ComposerError> {
    if ctx.is_empty() {
        return Ok(());
    }
    let parent_index: i32 = *ctx.last().unwrap();
    let parent = &mut document.nodes[parent_index as usize - 1];
    match parent.data {
        NodeData::Sequence { ref mut items, .. } => {
            items.push(index);
        }
        NodeData::Mapping { ref mut pairs, .. } => {
            let mut pair = NodePair::default();
            let mut do_push = true;
            if !pairs.is_empty() {
                let p: &mut NodePair = pairs.last_mut().unwrap();
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
    parser: &mut Parser,
    event: Event,
    document: &mut Document,
    ctx: &[i32],
) -> Result<(), ComposerError> {
    let EventData::Alias { anchor } = &event.data else {
        unreachable!()
    };

    for alias_data in &parser.aliases {
        if alias_data.anchor == *anchor {
            return yaml_parser_load_node_add(document, ctx, alias_data.index);
        }
    }

    yaml_parser_set_composer_error("found undefined alias", event.start_mark)
}

fn yaml_parser_load_scalar(
    parser: &mut Parser,
    event: Event,
    document: &mut Document,
    ctx: &[i32],
) -> Result<(), ComposerError> {
    let EventData::Scalar {
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
        tag = Some(String::from(DEFAULT_SCALAR_TAG));
    }
    let node = Node {
        data: NodeData::Scalar { value, style },
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
    parser: &mut Parser,
    event: Event,
    document: &mut Document,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    let EventData::SequenceStart {
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
        tag = Some(String::from(DEFAULT_SEQUENCE_TAG));
    }

    let node = Node {
        data: NodeData::Sequence {
            items: core::mem::take(&mut items),
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

fn yaml_parser_load_sequence_end(
    _parser: &mut Parser,
    event: Event,
    document: &mut Document,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    assert!(!ctx.is_empty());
    let index: i32 = *ctx.last().unwrap();
    assert!(matches!(
        document.nodes[index as usize - 1].data,
        NodeData::Sequence { .. }
    ));
    document.nodes[index as usize - 1].end_mark = event.end_mark;
    _ = ctx.pop();
    Ok(())
}

fn yaml_parser_load_mapping(
    parser: &mut Parser,
    event: Event,
    document: &mut Document,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    let EventData::MappingStart {
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
        tag = Some(String::from(DEFAULT_MAPPING_TAG));
    }
    let node = Node {
        data: NodeData::Mapping {
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
    _parser: &mut Parser,
    event: Event,
    document: &mut Document,
    ctx: &mut Vec<i32>,
) -> Result<(), ComposerError> {
    assert!(!ctx.is_empty());
    let index: i32 = *ctx.last().unwrap();
    assert!(matches!(
        document.nodes[index as usize - 1].data,
        NodeData::Mapping { .. }
    ));
    document.nodes[index as usize - 1].end_mark = event.end_mark;
    _ = ctx.pop();
    Ok(())
}
