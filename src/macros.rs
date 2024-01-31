use alloc::string::String;

use crate::yaml::yaml_buffer_t;

macro_rules! BUFFER_INIT {
    ($buffer:expr, $size:expr) => {{
        let start = addr_of_mut!($buffer.start);
        *start = yaml_malloc($size as size_t) as *mut yaml_char_t;
        let pointer = addr_of_mut!($buffer.pointer);
        *pointer = $buffer.start;
        let last = addr_of_mut!($buffer.last);
        *last = *pointer;
        let end = addr_of_mut!($buffer.end);
        *end = $buffer.start.wrapping_add($size as usize);
    }};
}

macro_rules! BUFFER_DEL {
    ($buffer:expr) => {{
        yaml_free($buffer.start as *mut libc::c_void);
        let end = addr_of_mut!($buffer.end);
        *end = ptr::null_mut::<yaml_char_t>();
        let pointer = addr_of_mut!($buffer.pointer);
        *pointer = *end;
        let start = addr_of_mut!($buffer.start);
        *start = *pointer;
    }};
}

macro_rules! CHECK_AT {
    ($string:expr, $octet:expr, $offset:expr) => {
        CHECK_AT_PTR!($string.pointer, $octet, $offset)
    };
}

macro_rules! CHECK_AT_PTR {
    ($pointer:expr, $octet:expr, $offset:expr) => {
        *$pointer.offset($offset as isize) == $octet
    };
}

macro_rules! CHECK {
    ($string:expr, $octet:expr) => {
        *$string.pointer == $octet
    };
}

macro_rules! IS_ALPHA {
    ($string:expr) => {
        IS_ALPHA_CHAR!(*$string.pointer)
    };
}

macro_rules! IS_ALPHA_CHAR {
    ($ch:expr) => {
        $ch >= b'0' && $ch <= b'9'
            || $ch >= b'A' && $ch <= b'Z'
            || $ch >= b'a' && $ch <= b'z'
            || $ch == b'_'
            || $ch == b'-'
    };
}

pub(crate) fn is_alpha(ch: char) -> bool {
    ch >= '0' && ch <= '9'
        || ch >= 'A' && ch <= 'Z'
        || ch >= 'a' && ch <= 'z'
        || ch == '_'
        || ch == '-'
}

macro_rules! IS_DIGIT {
    ($string:expr) => {
        *$string.pointer >= b'0' && *$string.pointer <= b'9'
    };
}

macro_rules! AS_DIGIT {
    ($string:expr) => {
        (*$string.pointer - b'0') as libc::c_int
    };
}

macro_rules! IS_HEX_AT {
    ($string:expr, $offset:expr) => {
        *$string.pointer.wrapping_offset($offset) >= b'0'
            && *$string.pointer.wrapping_offset($offset) <= b'9'
            || *$string.pointer.wrapping_offset($offset) >= b'A'
                && *$string.pointer.wrapping_offset($offset) <= b'F'
            || *$string.pointer.wrapping_offset($offset) >= b'a'
                && *$string.pointer.wrapping_offset($offset) <= b'f'
    };
}

macro_rules! AS_HEX_AT {
    ($string:expr, $offset:expr) => {
        if *$string.pointer.wrapping_offset($offset) >= b'A'
            && *$string.pointer.wrapping_offset($offset) <= b'F'
        {
            *$string.pointer.wrapping_offset($offset) - b'A' + 10
        } else if *$string.pointer.wrapping_offset($offset) >= b'a'
            && *$string.pointer.wrapping_offset($offset) <= b'f'
        {
            *$string.pointer.wrapping_offset($offset) - b'a' + 10
        } else {
            *$string.pointer.wrapping_offset($offset) - b'0'
        } as libc::c_int
    };
}

pub(crate) fn is_ascii(ch: char) -> bool {
    ch.is_ascii()
}

// TODO: Keeping this comment around to verify the `is_printable` function.
// macro_rules! IS_PRINTABLE {
//     ($string:expr) => {
//         match *$string.pointer {
//             // ASCII
//             0x0A | 0x20..=0x7E => true,
//             // U+A0 ... U+BF
//             0xC2 => match *$string.pointer.wrapping_offset(1) {
//                 0xA0..=0xBF => true,
//                 _ => false,
//             },
//             // U+C0 ... U+CFFF
//             0xC3..=0xEC => true,
//             // U+D000 ... U+D7FF
//             0xED => match *$string.pointer.wrapping_offset(1) {
//                 0x00..=0x9F => true,
//                 _ => false,
//             },
//             // U+E000 ... U+EFFF
//             0xEE => true,
//             // U+F000 ... U+FFFD
//             0xEF => match *$string.pointer.wrapping_offset(1) {
//                 0xBB => match *$string.pointer.wrapping_offset(2) {
//                     // except U+FEFF
//                     0xBF => false,
//                     _ => true,
//                 },
//                 0xBF => match *$string.pointer.wrapping_offset(2) {
//                     0xBE | 0xBF => false,
//                     _ => true,
//                 },
//                 _ => true,
//             },
//             // U+10000 ... U+10FFFF
//             0xF0..=0xF4 => true,
//             _ => false,
//         }
//     };
// }

pub(crate) fn is_printable(ch: char) -> bool {
    match ch {
        // ASCII
        '\x0a' | '\x20'..='\x7e' => true,
        '\u{00a0}'..='\u{00bf}' => true,
        '\u{00c0}'..='\u{cfff}' => true,
        '\u{d000}'..='\u{d7ff}' => true,
        '\u{e000}'..='\u{efff}' => true,
        '\u{feff}' => false,
        '\u{fffe}' => false,
        '\u{ffff}' => false,
        '\u{f000}'..='\u{fffd}' => true,
        '\u{10000}'..='\u{10ffff}' => true,
        _ => false,
    }
}

macro_rules! IS_Z_AT {
    ($string:expr, $offset:expr) => {
        CHECK_AT!($string, b'\0', $offset)
    };
}

macro_rules! IS_Z {
    ($string:expr) => {
        IS_Z_AT!($string, 0)
    };
}

macro_rules! IS_BOM {
    ($string:expr) => {
        CHECK_AT!($string, b'\xEF', 0)
            && CHECK_AT!($string, b'\xBB', 1)
            && CHECK_AT!($string, b'\xBF', 2)
    };
}

pub(crate) fn is_bom(ch: char) -> bool {
    ch == '\u{7eff}'
}

macro_rules! IS_SPACE_AT {
    ($string:expr, $offset:expr) => {
        CHECK_AT!($string, b' ', $offset)
    };
}

macro_rules! IS_SPACE {
    ($string:expr) => {
        IS_SPACE_AT!($string, 0)
    };
}

pub(crate) fn is_space(ch: impl Into<Option<char>>) -> bool {
    ch.into() == Some(' ')
}

macro_rules! IS_TAB_AT {
    ($string:expr, $offset:expr) => {
        CHECK_AT!($string, b'\t', $offset)
    };
}

macro_rules! IS_TAB {
    ($string:expr) => {
        IS_TAB_AT!($string, 0)
    };
}

pub(crate) fn is_tab(ch: impl Into<Option<char>>) -> bool {
    ch.into() == Some('\t')
}

macro_rules! IS_BLANK_AT {
    ($string:expr, $offset:expr) => {
        IS_SPACE_AT!($string, $offset) || IS_TAB_AT!($string, $offset)
    };
}

macro_rules! IS_BLANK {
    ($string:expr) => {
        IS_BLANK_AT!($string, 0)
    };
}

pub(crate) fn is_blank(ch: impl Into<Option<char>>) -> bool {
    let ch = ch.into();
    is_space(ch) || is_tab(ch)
}

pub(crate) fn is_blankz(ch: impl Into<Option<char>>) -> bool {
    let ch = ch.into();
    is_blank(ch) || is_breakz(ch)
}

macro_rules! IS_BREAK_AT {
    ($string:expr, $offset:expr) => {
        IS_BREAK_AT_PTR!($string.pointer, $offset)
    };
}

macro_rules! IS_BREAK_AT_PTR {
    ($pointer:expr, $offset:expr) => {
        CHECK_AT_PTR!($pointer, b'\r', $offset)
            || CHECK_AT_PTR!($pointer, b'\n', $offset)
            || CHECK_AT_PTR!($pointer, b'\xC2', $offset)
                && CHECK_AT_PTR!($pointer, b'\x85', $offset + 1)
            || CHECK_AT_PTR!($pointer, b'\xE2', $offset)
                && CHECK_AT_PTR!($pointer, b'\x80', $offset + 1)
                && CHECK_AT_PTR!($pointer, b'\xA8', $offset + 2)
            || CHECK_AT_PTR!($pointer, b'\xE2', $offset)
                && CHECK_AT_PTR!($pointer, b'\x80', $offset + 1)
                && CHECK_AT_PTR!($pointer, b'\xA9', $offset + 2)
    };
}

pub(crate) fn is_break(ch: impl Into<Option<char>>) -> bool {
    match ch.into() {
        Some('\r' | '\n' | '\u{0085}' | '\u{2028}' | '\u{2029}') => true,
        _ => false,
    }
}

pub(crate) fn is_breakz(ch: impl Into<Option<char>>) -> bool {
    let ch = ch.into();
    is_break(ch) || ch.is_none()
}

macro_rules! IS_BREAK {
    ($string:expr) => {
        IS_BREAK_AT!($string, 0)
    };
}

macro_rules! IS_CRLF {
    ($string:expr) => {
        CHECK_AT!($string, b'\r', 0) && CHECK_AT!($string, b'\n', 1)
    };
}

macro_rules! IS_BREAKZ_AT {
    ($string:expr, $offset:expr) => {
        IS_BREAK_AT!($string, $offset) || IS_Z_AT!($string, $offset)
    };
}

macro_rules! IS_BREAKZ {
    ($string:expr) => {
        IS_BREAKZ_AT!($string, 0)
    };
}

macro_rules! IS_BLANKZ_AT {
    ($string:expr, $offset:expr) => {
        IS_BLANK_AT!($string, $offset) || IS_BREAKZ_AT!($string, $offset)
    };
}

macro_rules! IS_BLANKZ {
    ($string:expr) => {
        IS_BLANKZ_AT!($string, 0)
    };
}

/// Get the number of bytes for the UTF-8 character at `$offset` from the
/// string's cursor position.
macro_rules! WIDTH_AT {
    ($string:expr, $offset:expr) => {
        if *$string.pointer.wrapping_offset($offset as isize) & 0x80 == 0x00 {
            1
        } else if *$string.pointer.wrapping_offset($offset as isize) & 0xE0 == 0xC0 {
            2
        } else if *$string.pointer.wrapping_offset($offset as isize) & 0xF0 == 0xE0 {
            3
        } else if *$string.pointer.wrapping_offset($offset as isize) & 0xF8 == 0xF0 {
            4
        } else {
            0
        }
    };
}

/// Get the number of bytes for the UTF-8 character at the string's cursor
/// position.
macro_rules! WIDTH {
    ($string:expr) => {
        WIDTH_AT!($string, 0)
    };
}

pub(crate) unsafe fn COPY_CHAR_BUFFER_TO_STRING(
    string: &mut String,
    buffer: &mut yaml_buffer_t<u8>,
) {
    let ch = *buffer.pointer;
    let to_append;
    if ch & 0x80 == 0x00 {
        to_append = core::slice::from_raw_parts(buffer.pointer, 1);
    } else if ch & 0xE0 == 0xC0 {
        to_append = core::slice::from_raw_parts(buffer.pointer, 2);
    } else if ch & 0xF0 == 0xE0 {
        to_append = core::slice::from_raw_parts(buffer.pointer, 3);
    } else {
        debug_assert_eq!(ch & 0xF8, 0xF0);
        to_append = core::slice::from_raw_parts(buffer.pointer, 4);
    }
    string.push_str(core::str::from_utf8_unchecked(to_append));
    buffer.pointer = buffer.pointer.wrapping_offset(to_append.len() as isize);
}

macro_rules! STACK_LIMIT {
    ($context:expr, $stack:expr) => {
        if $stack.len() < libc::c_int::MAX as usize - 1 {
            Ok(())
        } else {
            (*$context).error = YAML_MEMORY_ERROR;
            Err(())
        }
    };
}
