use crate::yaml_encoding_t::YAML_UTF16BE_ENCODING;
use crate::{
    yaml_emitter_t, WriterError, YAML_ANY_ENCODING, YAML_UTF16LE_ENCODING, YAML_UTF8_ENCODING,
};

/// Flush the accumulated characters to the output.
pub fn yaml_emitter_flush(emitter: &mut yaml_emitter_t) -> Result<(), WriterError> {
    __assert!((emitter.write_handler).is_some());
    __assert!(emitter.encoding != YAML_ANY_ENCODING);

    if emitter.buffer.is_empty() {
        return Ok(());
    }

    // TODO: Support partial writes. These calls fail unless the writer is able
    // to write absolutely everything in the buffer.

    if emitter.encoding == YAML_UTF8_ENCODING {
        let to_emit = emitter.buffer.as_bytes();
        if emitter
            .write_handler
            .as_mut()
            .expect("non-null writer")
            .write(to_emit)?
            == to_emit.len()
        {
            emitter.buffer.clear();
            return Ok(());
        } else {
            return Err(WriterError::Incomplete);
        }
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

    if emitter
        .write_handler
        .as_mut()
        .expect("non-null function pointer")
        .write(to_emit)?
        == to_emit.len()
    {
        emitter.buffer.clear();
        emitter.raw_buffer.clear();
        Ok(())
    } else {
        return Err(WriterError::Incomplete);
    }
}
