use crate::api::{yaml_free, yaml_malloc};
use crate::externs::{memset, strcmp};
use crate::fmt::WriteToPtr;
use crate::ops::ForceMul as _;
use crate::yaml::{
    yaml_anchors_t, yaml_char_t, yaml_document_t, yaml_emitter_t, yaml_event_t, yaml_node_item_t,
    yaml_node_pair_t, yaml_node_t, YamlEventData, YAML_ANY_ENCODING, YAML_MAPPING_NODE,
    YAML_SCALAR_NODE, YAML_SEQUENCE_NODE,
};
use crate::{libc, yaml_document_delete, yaml_emitter_emit, PointerExt};
use core::mem::size_of;
use core::ptr::{self, addr_of_mut};

/// Start a YAML stream.
///
/// This function should be used before yaml_emitter_dump() is called.
pub unsafe fn yaml_emitter_open(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    __assert!(!emitter.opened);
    let event = yaml_event_t {
        data: YamlEventData::StreamStart {
            encoding: YAML_ANY_ENCODING,
        },
        ..Default::default()
    };
    yaml_emitter_emit(emitter, &event)?;
    emitter.opened = true;
    Ok(())
}

/// Finish a YAML stream.
///
/// This function should be used after yaml_emitter_dump() is called.
pub unsafe fn yaml_emitter_close(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    __assert!(emitter.opened);
    if emitter.closed {
        return Ok(());
    }
    let event = yaml_event_t {
        data: YamlEventData::StreamEnd,
        ..Default::default()
    };
    yaml_emitter_emit(emitter, &event)?;
    emitter.closed = true;
    Ok(())
}

/// Emit a YAML document.
///
/// The documen object may be generated using the yaml_parser_load() function or
/// the yaml_document_initialize() function. The emitter takes the
/// responsibility for the document object and destroys its content after it is
/// emitted. The document object is destroyed even if the function fails.
pub unsafe fn yaml_emitter_dump(
    emitter: &mut yaml_emitter_t,
    document: *mut yaml_document_t,
) -> Result<(), ()> {
    let current_block: u64;
    __assert!(!document.is_null());
    emitter.document = document;
    if !emitter.opened {
        if yaml_emitter_open(emitter).is_err() {
            current_block = 5018439318894558507;
        } else {
            current_block = 15619007995458559411;
        }
    } else {
        current_block = 15619007995458559411;
    }
    match current_block {
        15619007995458559411 => {
            if STACK_EMPTY!((*document).nodes) {
                if let Ok(()) = yaml_emitter_close(emitter) {
                    yaml_emitter_delete_document_and_anchors(emitter);
                    return Ok(());
                }
            } else {
                __assert!(emitter.opened);
                emitter.anchors = yaml_malloc(
                    (size_of::<yaml_anchors_t>() as libc::c_ulong)
                        .force_mul((*document).nodes.top.c_offset_from((*document).nodes.start)
                            as libc::c_ulong),
                ) as *mut yaml_anchors_t;
                memset(
                    emitter.anchors as *mut libc::c_void,
                    0,
                    (size_of::<yaml_anchors_t>() as libc::c_ulong)
                        .force_mul((*document).nodes.top.c_offset_from((*document).nodes.start)
                            as libc::c_ulong),
                );
                let event = yaml_event_t {
                    data: YamlEventData::DocumentStart {
                        version_directive: (*document).version_directive,
                        tag_directives_start: (*document).tag_directives.start,
                        tag_directives_end: (*document).tag_directives.end,
                        implicit: (*document).start_implicit,
                    },
                    ..Default::default()
                };
                if let Ok(()) = yaml_emitter_emit(emitter, &event) {
                    yaml_emitter_anchor_node(emitter, 1);
                    if let Ok(()) = yaml_emitter_dump_node(emitter, 1) {
                        let event = yaml_event_t {
                            data: YamlEventData::DocumentEnd {
                                implicit: (*document).end_implicit,
                            },
                            ..Default::default()
                        };
                        if let Ok(()) = yaml_emitter_emit(emitter, &event) {
                            yaml_emitter_delete_document_and_anchors(emitter);
                            return Ok(());
                        }
                    }
                }
            }
        }
        _ => {}
    }
    yaml_emitter_delete_document_and_anchors(emitter);
    Err(())
}

unsafe fn yaml_emitter_delete_document_and_anchors(emitter: &mut yaml_emitter_t) {
    let mut index: libc::c_int;
    if emitter.anchors.is_null() {
        yaml_document_delete(&mut *emitter.document);
        emitter.document = ptr::null_mut::<yaml_document_t>();
        return;
    }
    index = 0;
    while (*emitter.document)
        .nodes
        .start
        .wrapping_offset(index as isize)
        < (*emitter.document).nodes.top
    {
        let mut node: yaml_node_t = *(*emitter.document)
            .nodes
            .start
            .wrapping_offset(index as isize);
        if !(*emitter.anchors.wrapping_offset(index as isize)).serialized {
            yaml_free(node.tag as *mut libc::c_void);
            if node.type_ == YAML_SCALAR_NODE {
                yaml_free(node.data.scalar.value as *mut libc::c_void);
            }
        }
        if node.type_ == YAML_SEQUENCE_NODE {
            STACK_DEL!(node.data.sequence.items);
        }
        if node.type_ == YAML_MAPPING_NODE {
            STACK_DEL!(node.data.mapping.pairs);
        }
        index += 1;
    }
    STACK_DEL!((*emitter.document).nodes);
    yaml_free(emitter.anchors as *mut libc::c_void);
    emitter.anchors = ptr::null_mut::<yaml_anchors_t>();
    emitter.last_anchor_id = 0;
    emitter.document = ptr::null_mut::<yaml_document_t>();
}

unsafe fn yaml_emitter_anchor_node_sub(emitter: &mut yaml_emitter_t, index: libc::c_int) {
    (*(emitter.anchors).offset((index - 1) as isize)).references += 1;
    if (*emitter.anchors.offset((index - 1) as isize)).references == 2 {
        emitter.last_anchor_id += 1;
        (*emitter.anchors.offset((index - 1) as isize)).anchor = emitter.last_anchor_id;
    }
}

unsafe fn yaml_emitter_anchor_node(emitter: &mut yaml_emitter_t, index: libc::c_int) {
    let node: *mut yaml_node_t = (*emitter.document)
        .nodes
        .start
        .wrapping_offset(index as isize)
        .wrapping_offset(-1_isize);
    let mut item: *mut yaml_node_item_t;
    let mut pair: *mut yaml_node_pair_t;
    let fresh8 =
        addr_of_mut!((*(emitter.anchors).wrapping_offset((index - 1) as isize)).references);
    *fresh8 += 1;
    if (*emitter.anchors.wrapping_offset((index - 1) as isize)).references == 1 {
        match (*node).type_ {
            YAML_SEQUENCE_NODE => {
                item = (*node).data.sequence.items.start;
                while item < (*node).data.sequence.items.top {
                    yaml_emitter_anchor_node_sub(emitter, *item);
                    item = item.wrapping_offset(1);
                }
            }
            YAML_MAPPING_NODE => {
                pair = (*node).data.mapping.pairs.start;
                while pair < (*node).data.mapping.pairs.top {
                    yaml_emitter_anchor_node_sub(emitter, (*pair).key);
                    yaml_emitter_anchor_node_sub(emitter, (*pair).value);
                    pair = pair.wrapping_offset(1);
                }
            }
            _ => {}
        }
    } else if (*emitter.anchors.wrapping_offset((index - 1) as isize)).references == 2 {
        let fresh9 = &mut emitter.last_anchor_id;
        *fresh9 += 1;
        (*emitter.anchors.wrapping_offset((index - 1) as isize)).anchor = *fresh9;
    }
}

unsafe fn yaml_emitter_generate_anchor(
    _emitter: &mut yaml_emitter_t,
    anchor_id: libc::c_int,
) -> *mut yaml_char_t {
    let anchor: *mut yaml_char_t = yaml_malloc(16_u64) as *mut yaml_char_t;
    write!(WriteToPtr::new(anchor), "id{:03}\0", anchor_id);
    anchor
}

unsafe fn yaml_emitter_dump_node(
    emitter: &mut yaml_emitter_t,
    index: libc::c_int,
) -> Result<(), ()> {
    let node: *mut yaml_node_t = (*emitter.document)
        .nodes
        .start
        .wrapping_offset(index as isize)
        .wrapping_offset(-1_isize);
    let anchor_id: libc::c_int = (*emitter.anchors.wrapping_offset((index - 1) as isize)).anchor;
    let mut anchor: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    if anchor_id != 0 {
        anchor = yaml_emitter_generate_anchor(emitter, anchor_id);
    }
    if (*emitter.anchors.wrapping_offset((index - 1) as isize)).serialized {
        return yaml_emitter_dump_alias(emitter, anchor);
    }
    (*emitter.anchors.wrapping_offset((index - 1) as isize)).serialized = true;
    match (*node).type_ {
        YAML_SCALAR_NODE => yaml_emitter_dump_scalar(emitter, node, anchor),
        YAML_SEQUENCE_NODE => yaml_emitter_dump_sequence(emitter, node, anchor),
        YAML_MAPPING_NODE => yaml_emitter_dump_mapping(emitter, node, anchor),
        _ => __assert!(false),
    }
}

unsafe fn yaml_emitter_dump_alias(
    emitter: &mut yaml_emitter_t,
    anchor: *mut yaml_char_t,
) -> Result<(), ()> {
    let event = yaml_event_t {
        data: YamlEventData::Alias { anchor },
        ..Default::default()
    };
    yaml_emitter_emit(emitter, &event)
}

unsafe fn yaml_emitter_dump_scalar(
    emitter: &mut yaml_emitter_t,
    node: *mut yaml_node_t,
    anchor: *mut yaml_char_t,
) -> Result<(), ()> {
    let plain_implicit = strcmp(
        (*node).tag as *mut libc::c_char,
        b"tag:yaml.org,2002:str\0" as *const u8 as *const libc::c_char,
    ) == 0;
    let quoted_implicit = strcmp(
        (*node).tag as *mut libc::c_char,
        b"tag:yaml.org,2002:str\0" as *const u8 as *const libc::c_char,
    ) == 0;

    let event = yaml_event_t {
        data: YamlEventData::Scalar {
            anchor,
            tag: (*node).tag,
            value: (*node).data.scalar.value,
            length: (*node).data.scalar.length,
            plain_implicit,
            quoted_implicit,
            style: (*node).data.scalar.style,
        },
        ..Default::default()
    };

    yaml_emitter_emit(emitter, &event)
}

unsafe fn yaml_emitter_dump_sequence(
    emitter: &mut yaml_emitter_t,
    node: *mut yaml_node_t,
    anchor: *mut yaml_char_t,
) -> Result<(), ()> {
    let implicit = strcmp(
        (*node).tag as *mut libc::c_char,
        b"tag:yaml.org,2002:seq\0" as *const u8 as *const libc::c_char,
    ) == 0;
    let mut item: *mut yaml_node_item_t;

    let event = yaml_event_t {
        data: YamlEventData::SequenceStart {
            anchor,
            tag: (*node).tag,
            implicit,
            style: (*node).data.sequence.style,
        },
        ..Default::default()
    };

    yaml_emitter_emit(emitter, &event)?;
    item = (*node).data.sequence.items.start;
    while item < (*node).data.sequence.items.top {
        yaml_emitter_dump_node(emitter, *item)?;
        item = item.wrapping_offset(1);
    }
    let event = yaml_event_t {
        data: YamlEventData::SequenceEnd,
        ..Default::default()
    };
    yaml_emitter_emit(emitter, &event)
}

unsafe fn yaml_emitter_dump_mapping(
    emitter: &mut yaml_emitter_t,
    node: *mut yaml_node_t,
    anchor: *mut yaml_char_t,
) -> Result<(), ()> {
    let implicit = strcmp(
        (*node).tag as *mut libc::c_char,
        b"tag:yaml.org,2002:map\0" as *const u8 as *const libc::c_char,
    ) == 0;
    let mut pair: *mut yaml_node_pair_t;

    let event = yaml_event_t {
        data: YamlEventData::MappingStart {
            anchor,
            tag: (*node).tag,
            implicit,
            style: (*node).data.mapping.style,
        },
        ..Default::default()
    };

    yaml_emitter_emit(emitter, &event)?;
    pair = (*node).data.mapping.pairs.start;
    while pair < (*node).data.mapping.pairs.top {
        yaml_emitter_dump_node(emitter, (*pair).key)?;
        yaml_emitter_dump_node(emitter, (*pair).value)?;
        pair = pair.wrapping_offset(1);
    }
    let event = yaml_event_t {
        data: YamlEventData::MappingEnd,
        ..Default::default()
    };
    yaml_emitter_emit(emitter, &event)
}
