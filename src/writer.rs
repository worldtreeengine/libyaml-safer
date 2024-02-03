use crate::Encoding::YAML_UTF16BE_ENCODING;
use crate::{Emitter, WriterError, YAML_ANY_ENCODING, YAML_UTF16LE_ENCODING, YAML_UTF8_ENCODING};

/// Flush the accumulated characters to the output.
pub fn yaml_emitter_flush(emitter: &mut Emitter) -> Result<(), WriterError> {
    assert!((emitter.write_handler).is_some());
    assert_ne!(emitter.encoding, YAML_ANY_ENCODING);

    if emitter.buffer.is_empty() {
        return Ok(());
    }

    // TODO: Support partial writes. These calls fail unless the writer is able
    // to write absolutely everything in the buffer.

    if emitter.encoding == YAML_UTF8_ENCODING {
        let to_emit = emitter.buffer.as_bytes();
        emitter
            .write_handler
            .as_mut()
            .expect("non-null writer")
            .write_all(to_emit)?;
        emitter.buffer.clear();
        return Ok(());
    }

    let big_endian = match emitter.encoding {
        YAML_ANY_ENCODING | YAML_UTF8_ENCODING => unreachable!("unhandled encoding"),
        YAML_UTF16LE_ENCODING => false,
        YAML_UTF16BE_ENCODING => true,
    };

    for ch in emitter.buffer.encode_utf16() {
        let bytes = if big_endian {
            ch.to_be_bytes()
        } else {
            ch.to_le_bytes()
        };
        emitter.raw_buffer.extend(bytes);
    }

    let to_emit = emitter.raw_buffer.as_slice();

    emitter
        .write_handler
        .as_mut()
        .expect("non-null function pointer")
        .write_all(to_emit)?;
    emitter.buffer.clear();
    emitter.raw_buffer.clear();
    Ok(())
}
