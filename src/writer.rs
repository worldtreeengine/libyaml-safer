use crate::ops::ForceAdd as _;
use crate::yaml::size_t;
use crate::{
    libc, yaml_emitter_t, PointerExt, YAML_ANY_ENCODING, YAML_UTF16LE_ENCODING, YAML_UTF8_ENCODING,
    YAML_WRITER_ERROR,
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
    emitter.buffer.last = emitter.buffer.pointer;
    emitter.buffer.pointer = emitter.buffer.start;
    if emitter.buffer.start == emitter.buffer.last {
        return Ok(());
    }
    if emitter.encoding == YAML_UTF8_ENCODING {
        if emitter.write_handler.expect("non-null function pointer")(
            emitter.write_handler_data,
            emitter.buffer.start,
            emitter.buffer.last.c_offset_from(emitter.buffer.start) as size_t,
        ) != 0
        {
            emitter.buffer.last = emitter.buffer.start;
            emitter.buffer.pointer = emitter.buffer.start;
            return Ok(());
        } else {
            return yaml_emitter_set_writer_error(emitter, "write error");
        }
    }
    let low: libc::c_int = if emitter.encoding == YAML_UTF16LE_ENCODING {
        0
    } else {
        1
    };
    let high: libc::c_int = if emitter.encoding == YAML_UTF16LE_ENCODING {
        1
    } else {
        0
    };
    while emitter.buffer.pointer != emitter.buffer.last {
        let mut octet: libc::c_uchar;
        let mut value: libc::c_uint;
        let mut k: size_t;
        octet = *emitter.buffer.pointer;
        let width: libc::c_uint = if octet & 0x80 == 0 {
            1
        } else if octet & 0xE0 == 0xC0 {
            2
        } else if octet & 0xF0 == 0xE0 {
            3
        } else if octet & 0xF8 == 0xF0 {
            4
        } else {
            0
        } as libc::c_uint;
        value = if octet & 0x80 == 0 {
            octet & 0x7F
        } else if octet & 0xE0 == 0xC0 {
            octet & 0x1F
        } else if octet & 0xF0 == 0xE0 {
            octet & 0xF
        } else if octet & 0xF8 == 0xF0 {
            octet & 0x7
        } else {
            0
        } as libc::c_uint;
        k = 1_u64;
        while k < width as libc::c_ulong {
            octet = *emitter.buffer.pointer.wrapping_offset(k as isize);
            value = (value << 6).force_add((octet & 0x3F) as libc::c_uint);
            k = k.force_add(1);
        }
        emitter.buffer.pointer = emitter.buffer.pointer.wrapping_offset(width as isize);
        if value < 0x10000 {
            *emitter.raw_buffer.last.wrapping_offset(high as isize) = (value >> 8) as libc::c_uchar;
            *emitter.raw_buffer.last.wrapping_offset(low as isize) =
                (value & 0xFF) as libc::c_uchar;
            emitter.raw_buffer.last = emitter.raw_buffer.last.wrapping_offset(2_isize);
        } else {
            value = value.wrapping_sub(0x10000);
            *emitter.raw_buffer.last.wrapping_offset(high as isize) =
                0xD8_u32.force_add(value >> 18) as libc::c_uchar;
            *emitter.raw_buffer.last.wrapping_offset(low as isize) =
                (value >> 10 & 0xFF) as libc::c_uchar;
            *(*emitter)
                .raw_buffer
                .last
                .wrapping_offset((high + 2) as isize) =
                0xDC_u32.force_add(value >> 8 & 0xFF) as libc::c_uchar;
            *(*emitter)
                .raw_buffer
                .last
                .wrapping_offset((low + 2) as isize) = (value & 0xFF) as libc::c_uchar;
            emitter.raw_buffer.last = emitter.raw_buffer.last.wrapping_offset(4_isize);
        }
    }
    if emitter.write_handler.expect("non-null function pointer")(
        emitter.write_handler_data,
        emitter.raw_buffer.start,
        (*emitter)
            .raw_buffer
            .last
            .c_offset_from(emitter.raw_buffer.start) as size_t,
    ) != 0
    {
        emitter.buffer.last = emitter.buffer.start;
        emitter.buffer.pointer = emitter.buffer.start;
        emitter.raw_buffer.last = emitter.raw_buffer.start;
        emitter.raw_buffer.pointer = emitter.raw_buffer.start;
        Ok(())
    } else {
        yaml_emitter_set_writer_error(emitter, "write error")
    }
}
