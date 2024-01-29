use alloc::{vec, vec::Vec};

use crate::api::{yaml_free, yaml_strdup};
use crate::externs::strcmp;
use crate::yaml::{yaml_char_t, YamlEventData, YamlNodeData};
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
    let current_block: u64;
    let mut event = yaml_event_t::default();
    core::ptr::write(document, yaml_document_t::default());
    let document = &mut *document;
    (*document).nodes.reserve(16);
    if !parser.stream_start_produced {
        if let Err(()) = yaml_parser_parse(parser, &mut event) {
            current_block = 6234624449317607669;
        } else {
            if let YamlEventData::StreamStart { .. } = &event.data {
            } else {
                panic!("expected stream start");
            }
            current_block = 7815301370352969686;
        }
    } else {
        current_block = 7815301370352969686;
    }
    if current_block != 6234624449317607669 {
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
    while let Some(alias) = parser.aliases.pop() {
        yaml_free(alias.anchor as *mut libc::c_void);
    }
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
    anchor: *mut yaml_char_t,
) -> Result<(), ()> {
    if anchor.is_null() {
        return Ok(());
    }
    let data = yaml_alias_data_t {
        anchor,
        index,
        mark: (*parser.document).nodes[index as usize - 1].start_mark,
    };
    for alias_data in parser.aliases.iter() {
        if strcmp(
            alias_data.anchor as *mut libc::c_char,
            anchor as *mut libc::c_char,
        ) == 0
        {
            yaml_free(anchor as *mut libc::c_void);
            return yaml_parser_set_composer_error_context(
                parser,
                "found duplicate anchor; first occurrence",
                (*alias_data).mark,
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
    let current_block_17: u64;
    match parent.data {
        YamlNodeData::Sequence { ref mut items, .. } => {
            STACK_LIMIT!(parser, items)?;
            items.push(index);
        }
        YamlNodeData::Mapping { ref mut pairs, .. } => {
            let mut pair = MaybeUninit::<yaml_node_pair_t>::uninit();
            let pair = pair.as_mut_ptr();
            if !pairs.is_empty() {
                let p: &mut yaml_node_pair_t = pairs.last_mut().unwrap();
                if p.key != 0 && p.value == 0 {
                    p.value = index;
                    current_block_17 = 11307063007268554308;
                } else {
                    current_block_17 = 17407779659766490442;
                }
            } else {
                current_block_17 = 17407779659766490442;
            }
            match current_block_17 {
                11307063007268554308 => {}
                _ => {
                    (*pair).key = index;
                    (*pair).value = 0;
                    STACK_LIMIT!(parser, pairs)?;
                    pairs.push(*pair);
                }
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
    let anchor: *mut yaml_char_t = if let YamlEventData::Alias { anchor } = &event.data {
        *anchor
    } else {
        unreachable!()
    };

    for alias_data in parser.aliases.iter() {
        if strcmp(
            alias_data.anchor as *mut libc::c_char,
            anchor as *mut libc::c_char,
        ) == 0
        {
            yaml_free(anchor as *mut libc::c_void);
            return yaml_parser_load_node_add(parser, ctx, (*alias_data).index);
        }
    }

    yaml_free(anchor as *mut libc::c_void);
    yaml_parser_set_composer_error(parser, "found undefined alias", (*event).start_mark)
}

unsafe fn yaml_parser_load_scalar(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t, // TODO: Take by value
    ctx: &mut Vec<libc::c_int>,
) -> Result<(), ()> {
    let (mut tag, value, length, style, anchor) = if let YamlEventData::Scalar {
        tag,
        value,
        length,
        style,
        anchor,
        ..
    } = &event.data
    {
        (*tag, *value, *length, *style, *anchor)
    } else {
        unreachable!()
    };

    let current_block: u64;
    let index: libc::c_int;
    if let Ok(()) = STACK_LIMIT!(parser, (*parser.document).nodes) {
        if tag.is_null()
            || strcmp(
                tag as *mut libc::c_char,
                b"!\0" as *const u8 as *const libc::c_char,
            ) == 0
        {
            yaml_free(tag as *mut libc::c_void);
            tag = yaml_strdup(
                b"tag:yaml.org,2002:str\0" as *const u8 as *const libc::c_char as *mut yaml_char_t,
            );
            if tag.is_null() {
                current_block = 10579931339944277179;
            } else {
                current_block = 11006700562992250127;
            }
        } else {
            current_block = 11006700562992250127;
        }
        if current_block != 10579931339944277179 {
            let node = yaml_node_t {
                data: YamlNodeData::Scalar {
                    value,
                    length,
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
    }
    yaml_free(tag as *mut libc::c_void);
    yaml_free(anchor as *mut libc::c_void);
    yaml_free(value as *mut libc::c_void);
    Err(())
}

unsafe fn yaml_parser_load_sequence(
    parser: &mut yaml_parser_t,
    event: &mut yaml_event_t, // TODO: Take by value.
    ctx: &mut Vec<libc::c_int>,
) -> Result<(), ()> {
    let (tag, style, anchor) = if let YamlEventData::SequenceStart {
        anchor, tag, style, ..
    } = &event.data
    {
        (*tag, *style, *anchor)
    } else {
        unreachable!()
    };

    let current_block: u64;

    let mut items = vec![];
    let index: libc::c_int;
    let mut tag: *mut yaml_char_t = tag;
    if let Ok(()) = STACK_LIMIT!(parser, (*parser.document).nodes) {
        if tag.is_null()
            || strcmp(
                tag as *mut libc::c_char,
                b"!\0" as *const u8 as *const libc::c_char,
            ) == 0
        {
            yaml_free(tag as *mut libc::c_void);
            tag = yaml_strdup(
                b"tag:yaml.org,2002:seq\0" as *const u8 as *const libc::c_char as *mut yaml_char_t,
            );
            if tag.is_null() {
                current_block = 13474536459355229096;
            } else {
                current_block = 6937071982253665452;
            }
        } else {
            current_block = 6937071982253665452;
        }
        if current_block != 13474536459355229096 {
            items.reserve(16);

            let node = yaml_node_t {
                data: YamlNodeData::Sequence {
                    items: core::mem::take(&mut items),
                    style,
                },
                tag,
                start_mark: (*event).start_mark,
                end_mark: (*event).end_mark,
            };

            (*parser.document).nodes.push(node);
            index = (*parser.document).nodes.len() as libc::c_int;
            yaml_parser_register_anchor(parser, index, anchor)?;
            yaml_parser_load_node_add(parser, ctx, index)?;
            STACK_LIMIT!(parser, *ctx)?;
            ctx.push(index);
            return Ok(());
        }
    }
    yaml_free(tag as *mut libc::c_void);
    yaml_free(anchor as *mut libc::c_void);
    Err(())
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
    let (tag, style, anchor) = if let YamlEventData::MappingStart {
        anchor, tag, style, ..
    } = &event.data
    {
        (*tag, *style, *anchor)
    } else {
        unreachable!()
    };

    let current_block: u64;

    let mut pairs = vec![];
    let index: libc::c_int;
    let mut tag: *mut yaml_char_t = tag;
    if let Ok(()) = STACK_LIMIT!(parser, (*parser.document).nodes) {
        if tag.is_null()
            || strcmp(
                tag as *mut libc::c_char,
                b"!\0" as *const u8 as *const libc::c_char,
            ) == 0
        {
            yaml_free(tag as *mut libc::c_void);
            tag = yaml_strdup(
                b"tag:yaml.org,2002:map\0" as *const u8 as *const libc::c_char as *mut yaml_char_t,
            );
            if tag.is_null() {
                current_block = 13635467803606088781;
            } else {
                current_block = 6937071982253665452;
            }
        } else {
            current_block = 6937071982253665452;
        }
        if current_block != 13635467803606088781 {
            pairs.reserve(16);
            let node = yaml_node_t {
                data: YamlNodeData::Mapping {
                    pairs: core::mem::take(&mut pairs),
                    style,
                },
                tag,
                start_mark: (*event).start_mark,
                end_mark: (*event).end_mark,
            };
            (*parser.document).nodes.push(node);
            index = (*parser.document).nodes.len() as libc::c_int;
            yaml_parser_register_anchor(parser, index, anchor)?;
            yaml_parser_load_node_add(parser, ctx, index)?;
            STACK_LIMIT!(parser, *ctx)?;
            ctx.push(index);
            return Ok(());
        }
    }
    yaml_free(tag as *mut libc::c_void);
    yaml_free(anchor as *mut libc::c_void);
    Err(())
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
