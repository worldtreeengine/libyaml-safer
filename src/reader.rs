use alloc::collections::VecDeque;

use crate::api::INPUT_RAW_BUFFER_SIZE;
use crate::macros::vecdeque_starts_with;
use crate::{
    yaml_parser_t, ReaderError, YAML_ANY_ENCODING, YAML_UTF16BE_ENCODING, YAML_UTF16LE_ENCODING,
    YAML_UTF8_ENCODING,
};

fn yaml_parser_set_reader_error<T>(
    _parser: &mut yaml_parser_t,
    problem: &'static str,
    offset: usize,
    value: i32,
) -> Result<T, ReaderError> {
    Err(ReaderError::Problem {
        problem,
        offset,
        value,
    })
}

const BOM_UTF8: &[u8] = b"\xEF\xBB\xBF";
const BOM_UTF16LE: &[u8] = b"\xFF\xFE";
const BOM_UTF16BE: &[u8] = b"\xFE\xFF";

fn yaml_parser_determine_encoding(parser: &mut yaml_parser_t) -> Result<(), ReaderError> {
    while !parser.eof && parser.raw_buffer.len() < 3 {
        yaml_parser_update_raw_buffer(parser)?;
    }
    if vecdeque_starts_with(&parser.raw_buffer, BOM_UTF16LE) {
        parser.encoding = YAML_UTF16LE_ENCODING;
        parser.raw_buffer.drain(0..2);
        parser.offset += 2;
    } else if vecdeque_starts_with(&parser.raw_buffer, BOM_UTF16BE) {
        parser.encoding = YAML_UTF16BE_ENCODING;
        parser.raw_buffer.drain(0..2);
        parser.offset += 2;
    } else if vecdeque_starts_with(&parser.raw_buffer, BOM_UTF8) {
        parser.encoding = YAML_UTF8_ENCODING;
        parser.raw_buffer.drain(0..3);
        parser.offset += 3;
    } else {
        parser.encoding = YAML_UTF8_ENCODING;
    }
    Ok(())
}

fn yaml_parser_update_raw_buffer(parser: &mut yaml_parser_t) -> Result<(), ReaderError> {
    if parser.raw_buffer.len() >= INPUT_RAW_BUFFER_SIZE {
        return Ok(());
    }
    if parser.eof {
        return Ok(());
    }

    let len_before = parser.raw_buffer.len();
    debug_assert!(len_before < INPUT_RAW_BUFFER_SIZE);
    parser.raw_buffer.resize(INPUT_RAW_BUFFER_SIZE, 0);
    let contiguous = parser.raw_buffer.make_contiguous();
    let write_to = &mut contiguous[len_before..];

    let size_read = parser
        .read_handler
        .as_mut()
        .expect("non-null read handler")
        .read(write_to)?;

    let valid_size = len_before + size_read;
    parser.raw_buffer.truncate(valid_size);
    if size_read == 0 {
        parser.eof = true;
    }
    Ok(())
}

fn utf8_char_width_and_initial_value(initial: u8) -> (usize, u32) {
    let initial = initial as u32;
    if initial & 0x80 == 0 {
        (1, initial & 0x7f)
    } else if initial & 0xE0 == 0xC0 {
        (2, initial & 0x1f)
    } else if initial & 0xF0 == 0xE0 {
        (3, initial & 0x0f)
    } else if initial & 0xF8 == 0xF0 {
        (4, initial & 0x07)
    } else {
        (0, 0)
    }
}

enum Utf8Error {
    Incomplete,
    InvalidLeadingOctet,
    InvalidTrailingOctet(usize),
    InvalidLength,
    InvalidUnicode(u32),
}

fn read_char_utf8(raw: &mut VecDeque<u8>) -> Option<Result<char, Utf8Error>> {
    let first = raw.front().copied()?;
    let (width, mut value) = utf8_char_width_and_initial_value(first);
    if width == 0 {
        return Some(Err(Utf8Error::InvalidLeadingOctet));
    }
    if raw.len() < width {
        return Some(Err(Utf8Error::Incomplete));
    }
    for (i, trailing) in raw.iter().enumerate().take(width).skip(1) {
        if trailing & 0xc0 != 0x80 {
            return Some(Err(Utf8Error::InvalidTrailingOctet(i)));
        }
        value <<= 6;
        value += *trailing as u32 & 0x3f;
    }
    if !(width == 1
        || width == 2 && value >= 0x80
        || width == 3 && value >= 0x800
        || width == 4 && value >= 0x10000)
    {
        return Some(Err(Utf8Error::InvalidLength));
    }
    if let Some(ch) = char::from_u32(value) {
        raw.drain(..width);
        Some(Ok(ch))
    } else {
        Some(Err(Utf8Error::InvalidUnicode(value)))
    }
}

enum Utf16Error {
    Incomplete,
    UnexpectedLowSurrogateArea(u32),
    ExpectedLowSurrogateArea(u32),
    InvalidUnicode(u32),
}

fn read_char_utf16<const BIG_ENDIAN: bool>(
    raw: &mut VecDeque<u8>,
) -> Option<Result<char, Utf16Error>> {
    if raw.is_empty() {
        return None;
    }
    if raw.len() < 2 {
        return Some(Err(Utf16Error::Incomplete));
    }
    let bytes = [raw[0], raw[1]];
    let mut value = if BIG_ENDIAN {
        u16::from_be_bytes(bytes) as u32
    } else {
        u16::from_le_bytes(bytes) as u32
    };
    if value & 0xfc00 == 0xdc00 {
        return Some(Err(Utf16Error::UnexpectedLowSurrogateArea(value)));
    }
    let width;
    if value & 0xfc00 == 0xd800 {
        width = 4;
        if raw.len() < width {
            return Some(Err(Utf16Error::Incomplete));
        }
        let bytes2 = [raw[2], raw[3]];
        let value2 = if BIG_ENDIAN {
            u16::from_be_bytes(bytes2) as u32
        } else {
            u16::from_le_bytes(bytes2) as u32
        };
        if value2 & 0xfc00 != 0xdc00 {
            return Some(Err(Utf16Error::ExpectedLowSurrogateArea(value2)));
        }
        value = (0x10000 + (value & 0x3ff)) << (10 + (value2 & 0x3ff));
    } else {
        width = 2;
    }

    if let Some(ch) = char::from_u32(value) {
        raw.drain(..width);
        Some(Ok(ch))
    } else {
        Some(Err(Utf16Error::InvalidUnicode(value)))
    }
}

fn push_char(parser: &mut yaml_parser_t, ch: char) -> Result<(), ReaderError> {
    if !(ch == '\x09'
        || ch == '\x0A'
        || ch == '\x0D'
        || ch >= '\x20' && ch <= '\x7E'
        || ch == '\u{0085}'
        || ch >= '\u{00A0}' && ch <= '\u{D7FF}'
        || ch >= '\u{E000}' && ch <= '\u{FFFD}'
        || ch >= '\u{10000}' && ch <= '\u{10FFFF}')
    {
        return yaml_parser_set_reader_error(
            parser,
            "control characters are not allowed",
            parser.offset,
            ch as _,
        );
    }
    parser.buffer.push_back(ch);
    parser.offset += ch.len_utf8();
    parser.unread += 1;
    Ok(())
}

pub(crate) fn yaml_parser_update_buffer(
    parser: &mut yaml_parser_t,
    length: usize,
) -> Result<(), ReaderError> {
    let mut first = true;
    assert!((parser.read_handler).is_some());
    if parser.eof && parser.raw_buffer.is_empty() {
        return Ok(());
    }
    if parser.unread >= length {
        return Ok(());
    }
    if parser.encoding == YAML_ANY_ENCODING {
        yaml_parser_determine_encoding(parser)?;
    }

    while parser.unread < length {
        if parser.eof && parser.raw_buffer.is_empty() {
            return Ok(());
        }
        if !first || parser.raw_buffer.is_empty() {
            yaml_parser_update_raw_buffer(parser)?;
        }
        first = false;
        match parser.encoding {
            YAML_UTF8_ENCODING => {
                match read_char_utf8(&mut parser.raw_buffer) {
                    Some(Ok(ch)) => {
                        push_char(parser, ch)?;
                    }
                    Some(Err(Utf8Error::Incomplete)) => {
                        if parser.eof {
                            return yaml_parser_set_reader_error(
                                parser,
                                "incomplete UTF-8 octet sequence",
                                parser.offset,
                                -1,
                            );
                        } else {
                            // Read more
                        }
                    }
                    Some(Err(Utf8Error::InvalidLeadingOctet)) => {
                        return yaml_parser_set_reader_error(
                            parser,
                            "invalid leading UTF-8 octet",
                            parser.offset,
                            parser.raw_buffer[0] as _,
                        );
                    }
                    Some(Err(Utf8Error::InvalidTrailingOctet(offset))) => {
                        return yaml_parser_set_reader_error(
                            parser,
                            "invalid trailing UTF-8 octet",
                            parser.offset + offset,
                            parser.raw_buffer[offset] as _,
                        );
                    }
                    Some(Err(Utf8Error::InvalidLength)) => {
                        return yaml_parser_set_reader_error(
                            parser,
                            "invalid length of a UTF-8 sequence",
                            parser.offset,
                            -1,
                        );
                    }
                    Some(Err(Utf8Error::InvalidUnicode(value))) => {
                        return yaml_parser_set_reader_error(
                            parser,
                            "invalid Unicode character",
                            parser.offset,
                            value as _,
                        );
                    }
                    None => (),
                }
            }
            YAML_UTF16LE_ENCODING | YAML_UTF16BE_ENCODING => {
                let is_big_endian = parser.encoding == YAML_UTF16BE_ENCODING;
                let res = if is_big_endian {
                    read_char_utf16::<true>(&mut parser.raw_buffer)
                } else {
                    read_char_utf16::<false>(&mut parser.raw_buffer)
                };
                match res {
                    Some(Ok(ch)) => {
                        push_char(parser, ch)?;
                    }
                    Some(Err(Utf16Error::Incomplete)) => {
                        if parser.eof {
                            return yaml_parser_set_reader_error(
                                parser,
                                "incomplete UTF-16 character",
                                parser.offset,
                                -1,
                            );
                        } else {
                            // Read more
                        }
                    }
                    Some(Err(Utf16Error::UnexpectedLowSurrogateArea(value))) => {
                        return yaml_parser_set_reader_error(
                            parser,
                            "unexpected low surrogate area",
                            parser.offset,
                            value as i32,
                        );
                    }
                    // Some(Err(Utf16Error::IncompleteSurrogatePair)) => {
                    //     return yaml_parser_set_reader_error(
                    //         parser,
                    //         "incomplete UTF-16 surrogate pair",
                    //         parser.offset,
                    //         -1,
                    //     );
                    // }
                    Some(Err(Utf16Error::ExpectedLowSurrogateArea(value))) => {
                        return yaml_parser_set_reader_error(
                            parser,
                            "expected low surrogate area",
                            parser.offset + 2,
                            value as i32,
                        );
                    }
                    Some(Err(Utf16Error::InvalidUnicode(value))) => {
                        return yaml_parser_set_reader_error(
                            parser,
                            "invalid Unicode character",
                            parser.offset,
                            value as i32,
                        );
                    }
                    None => (),
                }
            }
            _ => {
                panic!("unhandled encoded enum variant")
            }
        }
    }

    if parser.offset >= (!0_usize).wrapping_div(2_usize) {
        return yaml_parser_set_reader_error(parser, "input is too long", parser.offset, -1);
    }
    Ok(())
}
