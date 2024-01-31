use alloc::string::String;
use alloc::vec;

use crate::yaml::{
    yaml_anchors_t, yaml_document_t, yaml_emitter_t, yaml_event_t, yaml_node_t, YamlEventData,
    YamlNodeData, YAML_ANY_ENCODING,
};
use crate::{libc, yaml_document_delete, yaml_emitter_emit};

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
    yaml_emitter_emit(emitter, event)?;
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
    yaml_emitter_emit(emitter, event)?;
    emitter.closed = true;
    Ok(())
}

/// Emit a YAML document.
///
/// The documen object may be generated using the yaml_parser_load() function or
/// the yaml_document_initialize() function. The emitter takes the
/// responsibility for the document object and clears its content after it is
/// emitted. The document object is destroyed even if the function fails.
pub unsafe fn yaml_emitter_dump(
    emitter: &mut yaml_emitter_t,
    document: &mut yaml_document_t,
) -> Result<(), ()> {
    if !emitter.opened {
        if let Err(()) = yaml_emitter_open(emitter) {
            yaml_emitter_delete_document_and_anchors(emitter, document);
            return Err(());
        }
    }
    if document.nodes.is_empty() {
        yaml_emitter_close(emitter)?;
    } else {
        __assert!(emitter.opened);
        emitter.anchors = vec![yaml_anchors_t::default(); (*document).nodes.len()];
        let event = yaml_event_t {
            data: YamlEventData::DocumentStart {
                version_directive: (*document).version_directive,
                tag_directives: core::mem::take(&mut (*document).tag_directives),
                implicit: (*document).start_implicit,
            },
            ..Default::default()
        };
        yaml_emitter_emit(emitter, event)?;
        yaml_emitter_anchor_node(emitter, document, 1);
        yaml_emitter_dump_node(emitter, document, 1)?;
        let event = yaml_event_t {
            data: YamlEventData::DocumentEnd {
                implicit: document.end_implicit,
            },
            ..Default::default()
        };
        yaml_emitter_emit(emitter, event)?;
    }

    yaml_emitter_delete_document_and_anchors(emitter, document);
    Ok(())
}

unsafe fn yaml_emitter_delete_document_and_anchors(
    emitter: &mut yaml_emitter_t,
    document: &mut yaml_document_t,
) {
    if emitter.anchors.is_empty() {
        yaml_document_delete(document);
        return;
    }

    for (index, node) in document.nodes.iter_mut().enumerate() {
        if !emitter.anchors[index].serialized {
            // TODO: The `serialized` flag denoted that ownership of scalar and
            // tag was moved to someone else.
        }
        if let YamlNodeData::Sequence { ref mut items, .. } = node.data {
            items.clear();
        }
        if let YamlNodeData::Mapping { ref mut pairs, .. } = node.data {
            pairs.clear();
        }
    }

    document.nodes.clear();
    emitter.anchors.clear();
    emitter.last_anchor_id = 0;
}

unsafe fn yaml_emitter_anchor_node_sub(emitter: &mut yaml_emitter_t, index: libc::c_int) {
    emitter.anchors[index as usize - 1].references += 1;
    if emitter.anchors[index as usize - 1].references == 2 {
        emitter.last_anchor_id += 1;
        emitter.anchors[index as usize - 1].anchor = emitter.last_anchor_id;
    }
}

unsafe fn yaml_emitter_anchor_node(
    emitter: &mut yaml_emitter_t,
    document: &mut yaml_document_t,
    index: libc::c_int,
) {
    let node = &document.nodes[index as usize - 1];
    emitter.anchors[index as usize - 1].references += 1;
    if emitter.anchors[index as usize - 1].references == 1 {
        match &node.data {
            YamlNodeData::Sequence { items, .. } => {
                for item in items.iter() {
                    yaml_emitter_anchor_node_sub(emitter, *item);
                }
            }
            YamlNodeData::Mapping { pairs, .. } => {
                for pair in pairs.iter() {
                    yaml_emitter_anchor_node_sub(emitter, pair.key);
                    yaml_emitter_anchor_node_sub(emitter, pair.value);
                }
            }
            _ => {}
        }
    } else if emitter.anchors[index as usize - 1].references == 2 {
        emitter.last_anchor_id += 1;
        emitter.anchors[index as usize - 1].anchor = emitter.last_anchor_id;
    }
}

unsafe fn yaml_emitter_generate_anchor(
    _emitter: &mut yaml_emitter_t,
    anchor_id: libc::c_int,
) -> String {
    alloc::format!("id{:03}", anchor_id)
}

unsafe fn yaml_emitter_dump_node(
    emitter: &mut yaml_emitter_t,
    document: &mut yaml_document_t,
    index: libc::c_int,
) -> Result<(), ()> {
    let node = &mut document.nodes[index as usize - 1];
    let anchor_id: libc::c_int = emitter.anchors[index as usize - 1].anchor;
    let mut anchor: Option<String> = None;
    if anchor_id != 0 {
        anchor = Some(yaml_emitter_generate_anchor(emitter, anchor_id));
    }
    if emitter.anchors[index as usize - 1].serialized {
        return yaml_emitter_dump_alias(emitter, anchor.unwrap());
    }
    emitter.anchors[index as usize - 1].serialized = true;

    let node = core::mem::take(node);
    match node.data {
        YamlNodeData::Scalar { .. } => yaml_emitter_dump_scalar(emitter, node, anchor),
        YamlNodeData::Sequence { .. } => {
            yaml_emitter_dump_sequence(emitter, document, node, anchor)
        }
        YamlNodeData::Mapping { .. } => yaml_emitter_dump_mapping(emitter, document, node, anchor),
        _ => __assert!(false),
    }
}

unsafe fn yaml_emitter_dump_alias(emitter: &mut yaml_emitter_t, anchor: String) -> Result<(), ()> {
    let event = yaml_event_t {
        data: YamlEventData::Alias { anchor },
        ..Default::default()
    };
    yaml_emitter_emit(emitter, event)
}

unsafe fn yaml_emitter_dump_scalar(
    emitter: &mut yaml_emitter_t,
    node: yaml_node_t,
    anchor: Option<String>,
) -> Result<(), ()> {
    // TODO: Extract this constant as `YAML_DEFAULT_SCALAR_TAG` (source: dumper.c)
    let plain_implicit = node.tag.as_deref() == Some("tag:yaml.org,2002:str");
    let quoted_implicit = node.tag.as_deref() == Some("tag:yaml.org,2002:str"); // TODO: Why compare twice?! (even the C code does this)

    if let YamlNodeData::Scalar { value, style } = node.data {
        let event = yaml_event_t {
            data: YamlEventData::Scalar {
                anchor,
                tag: node.tag,
                value,
                plain_implicit,
                quoted_implicit,
                style,
            },
            ..Default::default()
        };
        yaml_emitter_emit(emitter, event)
    } else {
        unreachable!()
    }
}

unsafe fn yaml_emitter_dump_sequence(
    emitter: &mut yaml_emitter_t,
    document: &mut yaml_document_t,
    node: yaml_node_t,
    anchor: Option<String>,
) -> Result<(), ()> {
    // TODO: YAML_DEFAULT_SEQUENCE_TAG
    let implicit = node.tag.as_deref() == Some("tag:yaml.org,2002:seq");

    if let YamlNodeData::Sequence { items, style } = node.data {
        let event = yaml_event_t {
            data: YamlEventData::SequenceStart {
                anchor,
                tag: node.tag,
                implicit,
                style,
            },
            ..Default::default()
        };

        yaml_emitter_emit(emitter, event)?;
        for item in items {
            yaml_emitter_dump_node(emitter, document, item)?;
        }
        let event = yaml_event_t {
            data: YamlEventData::SequenceEnd,
            ..Default::default()
        };
        yaml_emitter_emit(emitter, event)
    } else {
        unreachable!()
    }
}

unsafe fn yaml_emitter_dump_mapping(
    emitter: &mut yaml_emitter_t,
    document: &mut yaml_document_t,
    node: yaml_node_t,
    anchor: Option<String>,
) -> Result<(), ()> {
    // TODO: YAML_DEFAULT_MAPPING_TAG
    let implicit = node.tag.as_deref() == Some("tag:yaml.org,2002:map");

    if let YamlNodeData::Mapping { pairs, style } = node.data {
        let event = yaml_event_t {
            data: YamlEventData::MappingStart {
                anchor,
                tag: node.tag,
                implicit,
                style,
            },
            ..Default::default()
        };

        yaml_emitter_emit(emitter, event)?;
        for pair in pairs {
            yaml_emitter_dump_node(emitter, document, pair.key)?;
            yaml_emitter_dump_node(emitter, document, pair.value)?;
        }
        let event = yaml_event_t {
            data: YamlEventData::MappingEnd,
            ..Default::default()
        };
        yaml_emitter_emit(emitter, event)
    } else {
        unreachable!()
    }
}
