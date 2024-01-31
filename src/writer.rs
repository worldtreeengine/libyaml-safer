use crate::yaml::size_t;
use crate::yaml_encoding_t::YAML_UTF16BE_ENCODING;
use crate::{
    yaml_emitter_t, YAML_ANY_ENCODING, YAML_UTF16LE_ENCODING, YAML_UTF8_ENCODING, YAML_WRITER_ERROR,
};

unsafe fn yaml_emitter_set_writer_error(
    emitter: &mut yaml_emitter_t,
    problem: &'static str,
) -> Result<(), ()> {
    emitter.error = YAML_WRITER_ERROR;
    emitter.problem = Some(problem);
    Err(())
}

/// Flush the accumulated characters to the output.
pub unsafe fn yaml_emitter_flush(emitter: &mut yaml_emitter_t) -> Result<(), ()> {
    __assert!((emitter.write_handler).is_some());
    __assert!(emitter.encoding != YAML_ANY_ENCODING);

    if emitter.buffer.is_empty() {
        return Ok(());
    }
    if emitter.encoding == YAML_UTF8_ENCODING {
        let to_emit = emitter.buffer.as_bytes();
        if emitter.write_handler.expect("non-null function pointer")(
            emitter.write_handler_data,
            to_emit.as_ptr(),
            to_emit.len() as size_t,
        ) != 0
        {
            emitter.buffer.clear();
            return Ok(());
        } else {
            return yaml_emitter_set_writer_error(emitter, "write error");
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

    if emitter.write_handler.expect("non-null function pointer")(
        emitter.write_handler_data,
        to_emit.as_ptr(),
        to_emit.len() as size_t,
    ) != 0
    {
        emitter.buffer.clear();
        emitter.raw_buffer.clear();
        Ok(())
    } else {
        yaml_emitter_set_writer_error(emitter, "write error")
    }
}
