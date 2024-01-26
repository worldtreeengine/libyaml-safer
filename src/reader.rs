use crate::externs::{memcmp, memmove};
use crate::ops::ForceAdd as _;
use crate::yaml::{size_t, yaml_char_t};
use crate::{
    libc, yaml_parser_t, PointerExt, YAML_ANY_ENCODING, YAML_READER_ERROR, YAML_UTF16BE_ENCODING,
    YAML_UTF16LE_ENCODING, YAML_UTF8_ENCODING,
};
use core::ptr::addr_of_mut;

unsafe fn yaml_parser_set_reader_error(
    parser: &mut yaml_parser_t,
    problem: &'static str,
    offset: size_t,
    value: libc::c_int,
) -> Result<(), ()> {
    parser.error = YAML_READER_ERROR;
    parser.problem = Some(problem);
    parser.problem_offset = offset;
    parser.problem_value = value;
    Err(())
}

const BOM_UTF8: *const libc::c_char = b"\xEF\xBB\xBF\0" as *const u8 as *const libc::c_char;
const BOM_UTF16LE: *const libc::c_char = b"\xFF\xFE\0" as *const u8 as *const libc::c_char;
const BOM_UTF16BE: *const libc::c_char = b"\xFE\xFF\0" as *const u8 as *const libc::c_char;

unsafe fn yaml_parser_determine_encoding(parser: &mut yaml_parser_t) -> Result<(), ()> {
    while !parser.eof
        && (parser
            .raw_buffer
            .last
            .c_offset_from(parser.raw_buffer.pointer) as libc::c_long)
            < 3_i64
    {
        yaml_parser_update_raw_buffer(parser)?;
    }
    if parser
        .raw_buffer
        .last
        .c_offset_from(parser.raw_buffer.pointer) as libc::c_long
        >= 2_i64
        && memcmp(
            parser.raw_buffer.pointer as *const libc::c_void,
            BOM_UTF16LE as *const libc::c_void,
            2_u64,
        ) == 0
    {
        parser.encoding = YAML_UTF16LE_ENCODING;
        parser.raw_buffer.pointer = parser.raw_buffer.pointer.wrapping_offset(2_isize);
        parser.offset = (parser.offset as libc::c_ulong).force_add(2_u64) as size_t;
    } else if parser
        .raw_buffer
        .last
        .c_offset_from(parser.raw_buffer.pointer) as libc::c_long
        >= 2_i64
        && memcmp(
            parser.raw_buffer.pointer as *const libc::c_void,
            BOM_UTF16BE as *const libc::c_void,
            2_u64,
        ) == 0
    {
        parser.encoding = YAML_UTF16BE_ENCODING;
        parser.raw_buffer.pointer = parser.raw_buffer.pointer.wrapping_offset(2_isize);
        parser.offset = (parser.offset as libc::c_ulong).force_add(2_u64) as size_t;
    } else if parser
        .raw_buffer
        .last
        .c_offset_from(parser.raw_buffer.pointer) as libc::c_long
        >= 3_i64
        && memcmp(
            parser.raw_buffer.pointer as *const libc::c_void,
            BOM_UTF8 as *const libc::c_void,
            3_u64,
        ) == 0
    {
        parser.encoding = YAML_UTF8_ENCODING;
        parser.raw_buffer.pointer = parser.raw_buffer.pointer.wrapping_offset(3_isize);
        parser.offset = (parser.offset as libc::c_ulong).force_add(3_u64) as size_t;
    } else {
        parser.encoding = YAML_UTF8_ENCODING;
    }
    Ok(())
}

unsafe fn yaml_parser_update_raw_buffer(parser: &mut yaml_parser_t) -> Result<(), ()> {
    let mut size_read: size_t = 0_u64;
    if parser.raw_buffer.start == parser.raw_buffer.pointer
        && parser.raw_buffer.last == parser.raw_buffer.end
    {
        return Ok(());
    }
    if parser.eof {
        return Ok(());
    }
    if parser.raw_buffer.start < parser.raw_buffer.pointer
        && parser.raw_buffer.pointer < parser.raw_buffer.last
    {
        memmove(
            parser.raw_buffer.start as *mut libc::c_void,
            parser.raw_buffer.pointer as *const libc::c_void,
            parser
                .raw_buffer
                .last
                .c_offset_from(parser.raw_buffer.pointer) as libc::c_long
                as libc::c_ulong,
        );
    }
    parser.raw_buffer.last = parser.raw_buffer.last.wrapping_offset(
        -(parser
            .raw_buffer
            .pointer
            .c_offset_from(parser.raw_buffer.start) as libc::c_long as isize),
    );
    parser.raw_buffer.pointer = parser.raw_buffer.start;
    if parser.read_handler.expect("non-null function pointer")(
        parser.read_handler_data,
        parser.raw_buffer.last,
        parser.raw_buffer.end.c_offset_from(parser.raw_buffer.last) as size_t,
        addr_of_mut!(size_read),
    ) == 0
    {
        return yaml_parser_set_reader_error(parser, "input error", parser.offset, -1);
    }
    parser.raw_buffer.last = parser.raw_buffer.last.wrapping_offset(size_read as isize);
    if size_read == 0 {
        parser.eof = true;
    }
    Ok(())
}

pub(crate) unsafe fn yaml_parser_update_buffer(
    parser: &mut yaml_parser_t,
    length: size_t,
) -> Result<(), ()> {
    let mut first = true;
    __assert!((parser.read_handler).is_some());
    if parser.eof && parser.raw_buffer.pointer == parser.raw_buffer.last {
        return Ok(());
    }
    if parser.unread >= length {
        return Ok(());
    }
    if parser.encoding == YAML_ANY_ENCODING {
        yaml_parser_determine_encoding(parser)?;
    }
    if parser.buffer.start < parser.buffer.pointer && parser.buffer.pointer < parser.buffer.last {
        let size: size_t = parser.buffer.last.c_offset_from(parser.buffer.pointer) as size_t;
        memmove(
            parser.buffer.start as *mut libc::c_void,
            parser.buffer.pointer as *const libc::c_void,
            size,
        );
        parser.buffer.pointer = parser.buffer.start;
        parser.buffer.last = parser.buffer.start.wrapping_offset(size as isize);
    } else if parser.buffer.pointer == parser.buffer.last {
        parser.buffer.pointer = parser.buffer.start;
        parser.buffer.last = parser.buffer.start;
    }
    while parser.unread < length {
        if !first || parser.raw_buffer.pointer == parser.raw_buffer.last {
            yaml_parser_update_raw_buffer(parser)?;
        }
        first = false;
        while parser.raw_buffer.pointer != parser.raw_buffer.last {
            let mut value: libc::c_uint = 0;
            let value2: libc::c_uint;
            let mut incomplete = false;
            let mut octet: libc::c_uchar;
            let mut width: libc::c_uint = 0;
            let low: libc::c_int;
            let high: libc::c_int;
            let mut k: size_t;
            let raw_unread: size_t = parser
                .raw_buffer
                .last
                .c_offset_from(parser.raw_buffer.pointer)
                as size_t;
            match parser.encoding {
                YAML_UTF8_ENCODING => {
                    octet = *parser.raw_buffer.pointer;
                    width = if octet & 0x80 == 0 {
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
                    if width == 0 {
                        return yaml_parser_set_reader_error(
                            parser,
                            "invalid leading UTF-8 octet",
                            parser.offset,
                            octet as libc::c_int,
                        );
                    }
                    if width as libc::c_ulong > raw_unread {
                        if parser.eof {
                            return yaml_parser_set_reader_error(
                                parser,
                                "incomplete UTF-8 octet sequence",
                                parser.offset,
                                -1,
                            );
                        }
                        incomplete = true;
                    } else {
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
                            octet = *parser.raw_buffer.pointer.wrapping_offset(k as isize);
                            if octet & 0xC0 != 0x80 {
                                return yaml_parser_set_reader_error(
                                    parser,
                                    "invalid trailing UTF-8 octet",
                                    parser.offset.force_add(k),
                                    octet as libc::c_int,
                                );
                            }
                            value = (value << 6).force_add((octet & 0x3F) as libc::c_uint);
                            k = k.force_add(1);
                        }
                        if !(width == 1
                            || width == 2 && value >= 0x80
                            || width == 3 && value >= 0x800
                            || width == 4 && value >= 0x10000)
                        {
                            return yaml_parser_set_reader_error(
                                parser,
                                "invalid length of a UTF-8 sequence",
                                parser.offset,
                                -1,
                            );
                        }
                        if value >= 0xD800 && value <= 0xDFFF || value > 0x10FFFF {
                            return yaml_parser_set_reader_error(
                                parser,
                                "invalid Unicode character",
                                parser.offset,
                                value as libc::c_int,
                            );
                        }
                    }
                }
                YAML_UTF16LE_ENCODING | YAML_UTF16BE_ENCODING => {
                    low = if parser.encoding == YAML_UTF16LE_ENCODING {
                        0
                    } else {
                        1
                    };
                    high = if parser.encoding == YAML_UTF16LE_ENCODING {
                        1
                    } else {
                        0
                    };
                    if raw_unread < 2_u64 {
                        if parser.eof {
                            return yaml_parser_set_reader_error(
                                parser,
                                "incomplete UTF-16 character",
                                parser.offset,
                                -1,
                            );
                        }
                        incomplete = true;
                    } else {
                        value = (*parser.raw_buffer.pointer.wrapping_offset(low as isize)
                            as libc::c_int
                            + ((*parser.raw_buffer.pointer.wrapping_offset(high as isize)
                                as libc::c_int)
                                << 8)) as libc::c_uint;
                        if value & 0xFC00 == 0xDC00 {
                            return yaml_parser_set_reader_error(
                                parser,
                                "unexpected low surrogate area",
                                parser.offset,
                                value as libc::c_int,
                            );
                        }
                        if value & 0xFC00 == 0xD800 {
                            width = 4;
                            if raw_unread < 4_u64 {
                                if parser.eof {
                                    return yaml_parser_set_reader_error(
                                        parser,
                                        "incomplete UTF-16 surrogate pair",
                                        parser.offset,
                                        -1,
                                    );
                                }
                                incomplete = true;
                            } else {
                                value2 = (*parser
                                    .raw_buffer
                                    .pointer
                                    .wrapping_offset((low + 2) as isize)
                                    as libc::c_int
                                    + ((*parser
                                        .raw_buffer
                                        .pointer
                                        .wrapping_offset((high + 2) as isize)
                                        as libc::c_int)
                                        << 8))
                                    as libc::c_uint;
                                if value2 & 0xFC00 != 0xDC00 {
                                    return yaml_parser_set_reader_error(
                                        parser,
                                        "expected low surrogate area",
                                        parser.offset.force_add(2_u64),
                                        value2 as libc::c_int,
                                    );
                                }
                                value = 0x10000_u32
                                    .force_add((value & 0x3FF) << 10)
                                    .force_add(value2 & 0x3FF);
                            }
                        } else {
                            width = 2;
                        }
                    }
                }
                _ => {}
            }
            if incomplete {
                break;
            }
            if !(value == 0x9
                || value == 0xA
                || value == 0xD
                || value >= 0x20 && value <= 0x7E
                || value == 0x85
                || value >= 0xA0 && value <= 0xD7FF
                || value >= 0xE000 && value <= 0xFFFD
                || value >= 0x10000 && value <= 0x10FFFF)
            {
                return yaml_parser_set_reader_error(
                    parser,
                    "control characters are not allowed",
                    parser.offset,
                    value as libc::c_int,
                );
            }
            parser.raw_buffer.pointer = parser.raw_buffer.pointer.wrapping_offset(width as isize);
            parser.offset =
                (parser.offset as libc::c_ulong).force_add(width as libc::c_ulong) as size_t;
            if value <= 0x7F {
                let q = parser.buffer.last;
                parser.buffer.last = parser.buffer.last.wrapping_offset(1);
                *q = value as yaml_char_t;
            } else if value <= 0x7FF {
                let q = *(&mut parser.buffer.last);
                parser.buffer.last = (parser.buffer.last).wrapping_offset(1);
                *q = 0xC0_u32.force_add(value >> 6) as yaml_char_t;
                let q = parser.buffer.last;
                parser.buffer.last = (parser.buffer.last).wrapping_offset(1);
                *q = 0x80_u32.force_add(value & 0x3F) as yaml_char_t;
            } else if value <= 0xFFFF {
                let q = parser.buffer.last;
                parser.buffer.last = (parser.buffer.last).wrapping_offset(1);
                *q = 0xE0_u32.force_add(value >> 12) as yaml_char_t;
                let q = parser.buffer.last;
                parser.buffer.last = (parser.buffer.last).wrapping_offset(1);
                *q = 0x80_u32.force_add(value >> 6 & 0x3F) as yaml_char_t;
                let q = parser.buffer.last;
                parser.buffer.last = (parser.buffer.last).wrapping_offset(1);
                *q = 0x80_u32.force_add(value & 0x3F) as yaml_char_t;
            } else {
                let q = parser.buffer.last;
                parser.buffer.last = (parser.buffer.last).wrapping_offset(1);
                *q = 0xF0_u32.force_add(value >> 18) as yaml_char_t;
                let q = parser.buffer.last;
                parser.buffer.last = (parser.buffer.last).wrapping_offset(1);
                *q = 0x80_u32.force_add(value >> 12 & 0x3F) as yaml_char_t;
                let q = parser.buffer.last;
                parser.buffer.last = (parser.buffer.last).wrapping_offset(1);
                *q = 0x80_u32.force_add(value >> 6 & 0x3F) as yaml_char_t;
                let q = parser.buffer.last;
                parser.buffer.last = (parser.buffer.last).wrapping_offset(1);
                *q = 0x80_u32.force_add(value & 0x3F) as yaml_char_t;
            }
            parser.unread = parser.unread.force_add(1);
        }
        if parser.eof {
            let p = &mut parser.buffer.last;
            let q = *p;
            *p = (*p).wrapping_offset(1);
            *q = b'\0';
            parser.unread = parser.unread.force_add(1);
            return Ok(());
        }
    }
    if parser.offset >= (!0_u64).wrapping_div(2_u64) {
        return yaml_parser_set_reader_error(parser, "input is too long", parser.offset, -1);
    }
    Ok(())
}
