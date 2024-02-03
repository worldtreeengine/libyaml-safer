use std::io::BufRead;

use alloc::collections::VecDeque;

use crate::{
    Encoding, Parser, ReaderError, YAML_ANY_ENCODING, YAML_UTF16BE_ENCODING, YAML_UTF16LE_ENCODING,
    YAML_UTF8_ENCODING,
};

fn yaml_parser_set_reader_error<T>(
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

const BOM_UTF8: [u8; 3] = [0xef, 0xbb, 0xbf];
const BOM_UTF16LE: [u8; 2] = [0xff, 0xfe];
const BOM_UTF16BE: [u8; 2] = [0xfe, 0xff];

fn yaml_parser_determine_encoding(
    reader: &mut dyn BufRead,
) -> Result<Option<Encoding>, ReaderError> {
    let initial_bytes = reader.fill_buf()?;
    if initial_bytes.is_empty() {
        return Ok(None);
    }

    match initial_bytes[0] {
        0xef => {
            let mut bom = [0; 3];
            reader.read_exact(&mut bom)?;
            if bom == BOM_UTF8 {
                Ok(Some(YAML_UTF8_ENCODING))
            } else {
                Err(ReaderError::InvalidBom)
            }
        }
        0xff | 0xfe => {
            let mut bom = [0; 2];
            reader.read_exact(&mut bom)?;
            if bom == BOM_UTF16LE {
                Ok(Some(YAML_UTF16LE_ENCODING))
            } else if bom == BOM_UTF16BE {
                Ok(Some(YAML_UTF16BE_ENCODING))
            } else {
                Err(ReaderError::InvalidBom)
            }
        }
        _ => Ok(Some(YAML_UTF8_ENCODING)),
    }
}

fn read_utf8_buffered(
    reader: &mut dyn BufRead,
    out: &mut VecDeque<char>,
    offset: &mut usize,
) -> Result<bool, ReaderError> {
    let available = loop {
        match reader.fill_buf() {
            Ok([]) => return Ok(false),
            Ok(available) => break available,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err.into()),
        }
    };

    match core::str::from_utf8(available) {
        Ok(valid) => {
            let used = valid.len();
            // The entire contents of the input buffer was valid UTF-8.
            for ch in valid.chars() {
                push_char(out, ch, *offset)?;
                *offset += ch.len_utf8();
            }
            reader.consume(used);
            Ok(true)
        }
        Err(err) => {
            let valid_bytes = err.valid_up_to();

            // If some of the buffer contents were valid, append that to the
            // output.
            let valid = unsafe {
                // SAFETY: This is safe because of `valid_up_to()`.
                core::str::from_utf8_unchecked(&available[..valid_bytes])
            };
            for ch in valid.chars() {
                push_char(out, ch, *offset)?;
                *offset += ch.len_utf8();
            }

            match err.error_len() {
                Some(_invalid_len) => {
                    return Err(ReaderError::InvalidUtf8 {
                        value: available[valid_bytes],
                    });
                }
                None => {
                    if valid_bytes != 0 {
                        // Some valid UTF-8 characters were present, and the
                        // tail end of the buffer was an incomplete sequence.
                        // Leave the incomplete sequence in the buffer.
                        reader.consume(valid_bytes);
                        Ok(true)
                    } else {
                        // The beginning of the buffer was an incomplete UTF-8
                        // sequence. Read the whole character unbuffered.
                        //
                        // This will return `UnexpectedEof` if the sequence
                        // cannot be completed. Note that `read_exact()` handles
                        // interrupt automatically.
                        let initial = available[0];
                        read_utf8_char_unbuffered(reader, out, initial, offset)?;
                        Ok(true)
                    }
                }
            }
        }
    }
}

fn read_utf8_char_unbuffered(
    reader: &mut dyn BufRead,
    out: &mut VecDeque<char>,
    initial: u8,
    offset: &mut usize,
) -> Result<(), ReaderError> {
    let width = utf8_char_width(initial);
    let mut buffer = [0; 4];
    reader.read_exact(&mut buffer[..width])?;
    if let Ok(valid) = core::str::from_utf8(&buffer[..width]) {
        // We read a whole, valid character.
        let Some(ch) = valid.chars().next() else {
            unreachable!()
        };
        push_char(out, ch, *offset)?;
        *offset += width;
        Ok(())
    } else {
        // Since we read the exact character width, the only
        // possible error here is invalid Unicode.
        Err(ReaderError::InvalidUtf8 { value: buffer[0] })
    }
}

fn read_utf16_buffered<const BIG_ENDIAN: bool>(
    reader: &mut dyn BufRead,
    out: &mut VecDeque<char>,
    offset: &mut usize,
) -> Result<bool, ReaderError> {
    let available = loop {
        match reader.fill_buf() {
            Ok([]) => return Ok(false),
            Ok(available) => break available,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err.into()),
        }
    };

    let chunks = available.chunks_exact(2).map(|chunk| {
        let [a, b] = chunk else { unreachable!() };
        if BIG_ENDIAN {
            u16::from_be_bytes([*a, *b])
        } else {
            u16::from_le_bytes([*a, *b])
        }
    });

    let mut used = 0;
    for ch in core::char::decode_utf16(chunks) {
        match ch {
            Ok(ch) => {
                push_char(out, ch, *offset)?;
                let n = ch.len_utf16();
                *offset += n;
                used += n;
            }
            Err(_) => {
                // An unpaired surrogate may either be a corrupt stream, but it
                // can also be that the buffer just happens to contain the first
                // half of a surrogate pair. Consume all of the valid bytes in
                // the buffer first, and then handle the unpaired surrogate in
                // the "slow" path (`read_utf16_char_unbuffered`) the next time
                // we are called.
                break;
            }
        }
    }

    if used != 0 {
        reader.consume(used);
        *offset += used;
        Ok(true)
    } else {
        debug_assert!(available.len() != 0 && available.len() < 2);
        read_utf16_char_unbuffered::<BIG_ENDIAN>(reader, out, offset)?;
        Ok(true)
    }
}

fn read_utf16_char_unbuffered<const BIG_ENDIAN: bool>(
    reader: &mut dyn BufRead,
    out: &mut VecDeque<char>,
    offset: &mut usize,
) -> Result<(), ReaderError> {
    let mut buffer = [0; 2];
    reader.read_exact(&mut buffer)?;
    let first = if BIG_ENDIAN {
        u16::from_be_bytes(buffer)
    } else {
        u16::from_le_bytes(buffer)
    };

    if is_utf16_surrogate(first) {
        reader.read_exact(&mut buffer)?;
        let second = if BIG_ENDIAN {
            u16::from_be_bytes(buffer)
        } else {
            u16::from_le_bytes(buffer)
        };

        match core::char::decode_utf16([first, second]).next() {
            Some(Ok(ch)) => {
                push_char(out, ch, *offset)?;
                *offset += 4;
                Ok(())
            }
            Some(Err(err)) => Err(ReaderError::InvalidUtf16 {
                value: err.unpaired_surrogate(),
            }),
            None => unreachable!(),
        }
    } else {
        match core::char::decode_utf16([first]).next() {
            Some(Ok(ch)) => {
                push_char(out, ch, *offset)?;
                *offset += 2;
                Ok(())
            }
            Some(Err(_)) | None => unreachable!(),
        }
    }
}

fn utf8_char_width(initial: u8) -> usize {
    if initial & 0x80 == 0 {
        1
    } else if initial & 0xE0 == 0xC0 {
        2
    } else if initial & 0xF0 == 0xE0 {
        3
    } else if initial & 0xF8 == 0xF0 {
        4
    } else {
        0
    }
}

fn is_utf16_surrogate(value: u16) -> bool {
    matches!(value, 0xD800..=0xDFFF)
}

fn push_char(out: &mut VecDeque<char>, ch: char, offset: usize) -> Result<(), ReaderError> {
    if !(ch == '\x09'
        || ch == '\x0A'
        || ch == '\x0D'
        || ch >= '\x20' && ch <= '\x7E'
        || ch == '\u{0085}'
        || ch >= '\u{00A0}' && ch <= '\u{D7FF}'
        || ch >= '\u{E000}' && ch <= '\u{FFFD}'
        || ch >= '\u{10000}' && ch <= '\u{10FFFF}')
    {
        return yaml_parser_set_reader_error("control characters are not allowed", offset, ch as _);
    }
    out.push_back(ch);
    Ok(())
}

pub(crate) fn yaml_parser_update_buffer(
    parser: &mut Parser,
    length: usize,
) -> Result<(), ReaderError> {
    let reader = parser.read_handler.as_deref_mut().expect("no read handler");
    if parser.unread >= length {
        return Ok(());
    }
    if parser.encoding == YAML_ANY_ENCODING {
        if let Some(encoding) = yaml_parser_determine_encoding(reader)? {
            parser.encoding = encoding;
        } else {
            parser.eof = true;
            return Ok(());
        }
    }

    while parser.unread < length {
        if parser.eof {
            return Ok(());
        }

        let tokens_before = parser.buffer.len();

        let not_eof = match parser.encoding {
            YAML_ANY_ENCODING => unreachable!(),
            YAML_UTF8_ENCODING => {
                read_utf8_buffered(reader, &mut parser.buffer, &mut parser.offset)?
            }
            YAML_UTF16LE_ENCODING => {
                read_utf16_buffered::<false>(reader, &mut parser.buffer, &mut parser.offset)?
            }
            YAML_UTF16BE_ENCODING => {
                read_utf16_buffered::<true>(reader, &mut parser.buffer, &mut parser.offset)?
            }
        };

        let num_read = parser.buffer.len() - tokens_before;
        parser.unread += num_read;
        if !not_eof {
            parser.eof = true;
            return Ok(());
        }
    }

    if parser.offset >= (!0_usize).wrapping_div(2_usize) {
        return yaml_parser_set_reader_error("input is too long", parser.offset, -1);
    }
    Ok(())
}
