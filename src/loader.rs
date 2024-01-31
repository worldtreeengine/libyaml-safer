use alloc::string::String;
use alloc::{vec, vec::Vec};

use crate::yaml::{YamlEventData, YamlNodeData};
use crate::{
    libc, yaml_alias_data_t, yaml_document_delete, yaml_document_t, yaml_event_t, yaml_mark_t,
    yaml_node_pair_t, yaml_node_t, yaml_parser_parse, yaml_parser_t, YAML_COMPOSER_ERROR,
    YAML_MEMORY_ERROR,
};
use core::mem::MaybeUninit;
use core::ptr::{self};

/// Parse the input stream and produce the next YAML document.
///
/// Call this function subsequently to produce a sequence of documents
/// constituting the input stream.
///
/// If the produced document has no root node, it means that the document end
/// has been reached.
///
/// An application is responsible for freeing any data associated with the
/// produced document object using the yaml_document_delete() function.
///
/// An application must not alternate the calls of yaml_parser_load() with the
/// calls of yaml_parser_scan() or yaml_parser_parse(). Doing this will break
/// the parser.
pub unsafe fn yaml_parser_load(
    parser: &mut yaml_parser_t,
    document: *mut yaml_document_t,
) -> Result<(), ()> {
    let mut event = yaml_event_t::default();
    core::ptr::write(document, yaml_document_t::default());
    let document = &mut *document;
    document.nodes.reserve(16);

    if !parser.stream_start_produced {
        if let Err(()) = yaml_parser_parse(parser, &mut event) {
            yaml_parser_delete_aliases(parser);
            yaml_document_delete(document);
            parser.document = ptr::null_mut::<yaml_document_t>();
            return Err(());
        } else {
            if let YamlEventData::StreamStart { .. } = &event.data {
            } else {
                panic!("expected stream start");
            }
        }
    }
    if parser.stream_end_produced {
        return Ok(());
    }
    if let Ok(()) = yaml_parser_parse(parser, &mut event) {
        if let YamlEventData::StreamEnd = &event.data {
            return Ok(());
        }
        parser.aliases.reserve(16);
        parser.document = document;
        if let Ok(()) = yaml_parser_load_document(parser, &mut event) {
            yaml_parser_delete_aliases(parser);
            parser.document = ptr::null_mut::<yaml_document_t>();
            return Ok(());
        }
    }
    yaml_parser_delete_aliases(parser);
    yaml_document_delete(document);
    parser.document = ptr::null_mut::<yaml_document_t>();
    Err(())
}

fn yaml_parser_set_composer_error(
    parser: &mut yaml_parser_t,
    problem: &'static str,
    problem_mark: yaml_mark_t,
) -> Result<(), ()> {
    parser.error = YAML_COMPOSER_ERROR;
    parser.problem = Some(problem);
    parser.problem_mark = problem_mark;
    Err(())
}

fn yaml_parser_set_composer_error_context(
    parser: &mut yaml_parser_t,
    context: &'static str,
    context_mark: yaml_mark_t,
    problem: &'static str,
    problem_mark: yaml_mark_t,
) -> Result<(), ()> {
    parser.error = YAML_COMPOSER_ERROR;
    parser.context = Some(context);
    parser.context_mark = context_mark;
    parser.problem = Some(problem);
    parser.problem_mark = problem_mark;
    Err(())
}

unsafe fn yaml_parser_delete_aliases(parser: &mut yaml_parser_t) {
    parser.aliases.clear();
}

unsafe fn yaml_parser_load_document(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t,
) -> Result<(), ()> {
    let mut ctx = vec![];
    if let YamlEventData::DocumentStart {
        version_directive,
        tag_directives,
        implicit,
    } = &mut event.data
    {
        (*parser.document).version_directive = *version_directive;
        (*parser.document).tag_directives = core::mem::take(tag_directives);
        (*parser.document).start_implicit = *implicit;
        (*parser.document).start_mark = event.start_mark;
        ctx.reserve(16);
        if let Err(()) = yaml_parser_load_nodes(parser, &mut ctx) {
            ctx.clear();
            return Err(());
        }
        ctx.clear();
        Ok(())
    } else {
        crate::externs::__assert_fail("event.type_ == YAML_DOCUMENT_START_EVENT", file!(), line!())
    }
}

unsafe fn yaml_parser_load_nodes(
    parser: &mut yaml_parser_t,
    ctx: &mut Vec<libc::c_int>,
) -> Result<(), ()> {
    let mut event = yaml_event_t::default();
    let end_implicit;
    let end_mark;

    loop {
        yaml_parser_parse(parser, &mut event)?;
        match &event.data {
            YamlEventData::NoEvent => panic!("empty event"),
            YamlEventData::StreamStart { .. } => panic!("unexpected stream start event"),
            YamlEventData::StreamEnd => panic!("unexpected stream end event"),
            YamlEventData::DocumentStart { .. } => panic!("unexpected document start event"),
            YamlEventData::DocumentEnd { implicit } => {
                end_implicit = *implicit;
                end_mark = event.end_mark;
                break;
            }
            YamlEventData::Alias { .. } => {
                yaml_parser_load_alias(parser, &mut event, ctx)?;
            }
            YamlEventData::Scalar { .. } => {
                yaml_parser_load_scalar(parser, &mut event, ctx)?;
            }
            YamlEventData::SequenceStart { .. } => {
                yaml_parser_load_sequence(parser, &mut event, ctx)?;
            }
            YamlEventData::SequenceEnd => {
                yaml_parser_load_sequence_end(parser, &mut event, ctx)?;
            }
            YamlEventData::MappingStart { .. } => {
                yaml_parser_load_mapping(parser, &mut event, ctx)?;
            }
            YamlEventData::MappingEnd => {
                yaml_parser_load_mapping_end(parser, &mut event, ctx)?;
            }
        }
    }
    (*parser.document).end_implicit = end_implicit;
    (*parser.document).end_mark = end_mark;
    Ok(())
}

unsafe fn yaml_parser_register_anchor(
    parser: &mut yaml_parser_t,
    index: libc::c_int,
    anchor: Option<String>,
) -> Result<(), ()> {
    let Some(anchor) = anchor else {
        return Ok(());
    };
    let data = yaml_alias_data_t {
        anchor,
        index,
        mark: (*parser.document).nodes[index as usize - 1].start_mark,
    };
    for alias_data in parser.aliases.iter() {
        if alias_data.anchor == data.anchor {
            return yaml_parser_set_composer_error_context(
                parser,
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

unsafe fn yaml_parser_load_node_add(
    parser: &mut yaml_parser_t,
    ctx: &mut Vec<libc::c_int>,
    index: libc::c_int,
) -> Result<(), ()> {
    if ctx.is_empty() {
        return Ok(());
    }
    let parent_index: libc::c_int = *ctx.last().unwrap();
    let parent = &mut (*parser.document).nodes[parent_index as usize - 1];
    match parent.data {
        YamlNodeData::Sequence { ref mut items, .. } => {
            STACK_LIMIT!(parser, items)?;
            items.push(index);
        }
        YamlNodeData::Mapping { ref mut pairs, .. } => {
            let mut pair = MaybeUninit::<yaml_node_pair_t>::uninit();
            let pair = pair.as_mut_ptr();
            let mut do_push = true;
            if !pairs.is_empty() {
                let p: &mut yaml_node_pair_t = pairs.last_mut().unwrap();
                if p.key != 0 && p.value == 0 {
                    p.value = index;
                    do_push = false;
                }
            }
            if do_push {
                (*pair).key = index;
                (*pair).value = 0;
                STACK_LIMIT!(parser, pairs)?;
                pairs.push(*pair);
            }
        }
        _ => {
            __assert!(false);
        }
    }
    Ok(())
}

unsafe fn yaml_parser_load_alias(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t, // TODO: Take by value
    ctx: &mut Vec<libc::c_int>,
) -> Result<(), ()> {
    let anchor: &str = if let YamlEventData::Alias { anchor } = &event.data {
        &*anchor
    } else {
        unreachable!()
    };

    for alias_data in parser.aliases.iter() {
        if alias_data.anchor == anchor {
            return yaml_parser_load_node_add(parser, ctx, alias_data.index);
        }
    }

    yaml_parser_set_composer_error(parser, "found undefined alias", event.start_mark)
}

unsafe fn yaml_parser_load_scalar(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t, // TODO: Take by value
    ctx: &mut Vec<libc::c_int>,
) -> Result<(), ()> {
    let (mut tag, value, style, anchor) = if let YamlEventData::Scalar {
        tag,
        value,
        style,
        anchor,
        ..
    } = &event.data
    {
        (tag.clone(), value, *style, anchor.clone())
    } else {
        unreachable!()
    };

    let index: libc::c_int;
    if let Ok(()) = STACK_LIMIT!(parser, (*parser.document).nodes) {
        if tag.is_none() || tag.as_deref() == Some("!") {
            tag = Some(String::from("tag:yaml.org,2002:str"));
        }
        let node = yaml_node_t {
            data: YamlNodeData::Scalar {
                value: value.clone(), // TODO: move
                style,
            },
            tag,
            start_mark: (*event).start_mark,
            end_mark: (*event).end_mark,
        };
        (*parser.document).nodes.push(node);
        index = (*parser.document).nodes.len() as libc::c_int;
        yaml_parser_register_anchor(parser, index, anchor)?;
        return yaml_parser_load_node_add(parser, ctx, index);
    }
    Err(())
}

unsafe fn yaml_parser_load_sequence(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t, // TODO: Take by value.
    ctx: &mut Vec<libc::c_int>,
) -> Result<(), ()> {
    let (mut tag, style, anchor) = if let YamlEventData::SequenceStart {
        anchor, tag, style, ..
    } = &event.data
    {
        (tag.clone(), *style, anchor)
    } else {
        unreachable!()
    };

    let mut items = Vec::with_capacity(16);
    let index: libc::c_int;
    STACK_LIMIT!(parser, (*parser.document).nodes)?;
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

    (*parser.document).nodes.push(node);
    index = (*parser.document).nodes.len() as libc::c_int;
    yaml_parser_register_anchor(parser, index, anchor.clone())?;
    yaml_parser_load_node_add(parser, ctx, index)?;
    STACK_LIMIT!(parser, *ctx)?;
    ctx.push(index);
    Ok(())
}

unsafe fn yaml_parser_load_sequence_end(
    parser: &mut yaml_parser_t,
    event: *mut yaml_event_t,
    ctx: &mut Vec<libc::c_int>,
) -> Result<(), ()> {
    __assert!(!ctx.is_empty());
    let index: libc::c_int = *ctx.last().unwrap();
    __assert!(matches!(
        (*parser.document).nodes[index as usize - 1].data,
        YamlNodeData::Sequence { .. }
    ));
    (*parser.document).nodes[index as usize - 1].end_mark = (*event).end_mark;
    _ = ctx.pop();
    Ok(())
}

unsafe fn yaml_parser_load_mapping(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t, // TODO: take by value
    ctx: &mut Vec<libc::c_int>,
) -> Result<(), ()> {
    let (mut tag, style, anchor) = if let YamlEventData::MappingStart {
        anchor, tag, style, ..
    } = &event.data
    {
        (tag.clone(), *style, anchor.clone())
    } else {
        unreachable!()
    };

    let mut pairs = Vec::with_capacity(16);
    let index: libc::c_int;
    STACK_LIMIT!(parser, (*parser.document).nodes)?;
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
    (*parser.document).nodes.push(node);
    index = (*parser.document).nodes.len() as libc::c_int;
    yaml_parser_register_anchor(parser, index, anchor)?;
    yaml_parser_load_node_add(parser, ctx, index)?;
    STACK_LIMIT!(parser, *ctx)?;
    ctx.push(index);
    Ok(())
}

unsafe fn yaml_parser_load_mapping_end(
    parser: &mut yaml_parser_t,
    event: *mut yaml_event_t,
    ctx: &mut Vec<libc::c_int>,
) -> Result<(), ()> {
    __assert!(!ctx.is_empty());
    let index: libc::c_int = *ctx.last().unwrap();
    __assert!(matches!(
        (*parser.document).nodes[index as usize - 1].data,
        YamlNodeData::Mapping { .. }
    ));
    (*parser.document).nodes[index as usize - 1].end_mark = (*event).end_mark;
    _ = ctx.pop();
    Ok(())
}
