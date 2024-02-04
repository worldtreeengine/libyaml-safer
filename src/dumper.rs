use std::mem::take;

use alloc::string::String;
use alloc::vec;

use crate::yaml::{Anchors, Any, Document, Emitter, Event, EventData, Node, NodeData};
use crate::{
    yaml_emitter_emit, EmitterError, DEFAULT_MAPPING_TAG, DEFAULT_SCALAR_TAG, DEFAULT_SEQUENCE_TAG,
};

/// Start a YAML stream.
///
/// This function should be used before
/// [`yaml_emitter_dump()`](crate::yaml_emitter_dump) is called.
pub fn yaml_emitter_open(emitter: &mut Emitter) -> Result<(), EmitterError> {
    assert!(!emitter.opened);
    let event = Event {
        data: EventData::StreamStart { encoding: Any },
        ..Default::default()
    };
    yaml_emitter_emit(emitter, event)?;
    emitter.opened = true;
    Ok(())
}

/// Finish a YAML stream.
///
/// This function should be used after
/// [`yaml_emitter_dump()`](crate::yaml_emitter_dump) is called.
pub fn yaml_emitter_close(emitter: &mut Emitter) -> Result<(), EmitterError> {
    assert!(emitter.opened);
    if emitter.closed {
        return Ok(());
    }
    let event = Event {
        data: EventData::StreamEnd,
        ..Default::default()
    };
    yaml_emitter_emit(emitter, event)?;
    emitter.closed = true;
    Ok(())
}

/// Emit a YAML document.
///
/// The document object may be generated using the
/// [`yaml_parser_load()`](crate::yaml_parser_load) function or the
/// [`yaml_document_new()`](crate::yaml_document_new) function.
pub fn yaml_emitter_dump(
    emitter: &mut Emitter,
    mut document: Document,
) -> Result<(), EmitterError> {
    if !emitter.opened {
        if let Err(err) = yaml_emitter_open(emitter) {
            yaml_emitter_reset_anchors(emitter);
            return Err(err);
        }
    }
    if document.nodes.is_empty() {
        yaml_emitter_close(emitter)?;
    } else {
        assert!(emitter.opened);
        emitter.anchors = vec![Anchors::default(); document.nodes.len()];
        let event = Event {
            data: EventData::DocumentStart {
                version_directive: document.version_directive,
                tag_directives: take(&mut document.tag_directives),
                implicit: document.start_implicit,
            },
            ..Default::default()
        };
        yaml_emitter_emit(emitter, event)?;
        yaml_emitter_anchor_node(emitter, &document, 1);
        yaml_emitter_dump_node(emitter, &mut document, 1)?;
        let event = Event {
            data: EventData::DocumentEnd {
                implicit: document.end_implicit,
            },
            ..Default::default()
        };
        yaml_emitter_emit(emitter, event)?;
    }

    yaml_emitter_reset_anchors(emitter);
    Ok(())
}

fn yaml_emitter_reset_anchors(emitter: &mut Emitter) {
    emitter.anchors.clear();
    emitter.last_anchor_id = 0;
}

fn yaml_emitter_anchor_node_sub(emitter: &mut Emitter, index: i32) {
    emitter.anchors[index as usize - 1].references += 1;
    if emitter.anchors[index as usize - 1].references == 2 {
        emitter.last_anchor_id += 1;
        emitter.anchors[index as usize - 1].anchor = emitter.last_anchor_id;
    }
}

fn yaml_emitter_anchor_node(emitter: &mut Emitter, document: &Document, index: i32) {
    let node = &document.nodes[index as usize - 1];
    emitter.anchors[index as usize - 1].references += 1;
    if emitter.anchors[index as usize - 1].references == 1 {
        match &node.data {
            NodeData::Sequence { items, .. } => {
                for item in items {
                    yaml_emitter_anchor_node_sub(emitter, *item);
                }
            }
            NodeData::Mapping { pairs, .. } => {
                for pair in pairs {
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

fn yaml_emitter_generate_anchor(_emitter: &mut Emitter, anchor_id: i32) -> String {
    alloc::format!("id{anchor_id:03}")
}

fn yaml_emitter_dump_node(
    emitter: &mut Emitter,
    document: &mut Document,
    index: i32,
) -> Result<(), EmitterError> {
    let node = &mut document.nodes[index as usize - 1];
    let anchor_id: i32 = emitter.anchors[index as usize - 1].anchor;
    let mut anchor: Option<String> = None;
    if anchor_id != 0 {
        anchor = Some(yaml_emitter_generate_anchor(emitter, anchor_id));
    }
    if emitter.anchors[index as usize - 1].serialized {
        return yaml_emitter_dump_alias(emitter, anchor.unwrap());
    }
    emitter.anchors[index as usize - 1].serialized = true;

    let node = take(node);
    match node.data {
        NodeData::Scalar { .. } => yaml_emitter_dump_scalar(emitter, node, anchor),
        NodeData::Sequence { .. } => yaml_emitter_dump_sequence(emitter, document, node, anchor),
        NodeData::Mapping { .. } => yaml_emitter_dump_mapping(emitter, document, node, anchor),
        _ => unreachable!("document node is neither a scalar, sequence, or a mapping"),
    }
}

fn yaml_emitter_dump_alias(emitter: &mut Emitter, anchor: String) -> Result<(), EmitterError> {
    let event = Event {
        data: EventData::Alias { anchor },
        ..Default::default()
    };
    yaml_emitter_emit(emitter, event)
}

fn yaml_emitter_dump_scalar(
    emitter: &mut Emitter,
    node: Node,
    anchor: Option<String>,
) -> Result<(), EmitterError> {
    let plain_implicit = node.tag.as_deref() == Some(DEFAULT_SCALAR_TAG);
    let quoted_implicit = node.tag.as_deref() == Some(DEFAULT_SCALAR_TAG); // TODO: Why compare twice?! (even the C code does this)

    if let NodeData::Scalar { value, style } = node.data {
        let event = Event {
            data: EventData::Scalar {
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

fn yaml_emitter_dump_sequence(
    emitter: &mut Emitter,
    document: &mut Document,
    node: Node,
    anchor: Option<String>,
) -> Result<(), EmitterError> {
    let implicit = node.tag.as_deref() == Some(DEFAULT_SEQUENCE_TAG);

    if let NodeData::Sequence { items, style } = node.data {
        let event = Event {
            data: EventData::SequenceStart {
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
        let event = Event {
            data: EventData::SequenceEnd,
            ..Default::default()
        };
        yaml_emitter_emit(emitter, event)
    } else {
        unreachable!()
    }
}

fn yaml_emitter_dump_mapping(
    emitter: &mut Emitter,
    document: &mut Document,
    node: Node,
    anchor: Option<String>,
) -> Result<(), EmitterError> {
    let implicit = node.tag.as_deref() == Some(DEFAULT_MAPPING_TAG);

    if let NodeData::Mapping { pairs, style } = node.data {
        let event = Event {
            data: EventData::MappingStart {
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
        let event = Event {
            data: EventData::MappingEnd,
            ..Default::default()
        };
        yaml_emitter_emit(emitter, event)
    } else {
        unreachable!()
    }
}
