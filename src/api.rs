use crate::externs::{free, malloc, memcpy, memmove, memset, realloc, strdup, strlen};
use crate::ops::{ForceAdd as _, ForceMul as _};
use crate::yaml::{size_t, yaml_char_t, YamlEventData, YamlNodeData, YamlTokenData};
use crate::{
    libc, yaml_break_t, yaml_document_t, yaml_emitter_state_t, yaml_emitter_t, yaml_encoding_t,
    yaml_event_t, yaml_mapping_style_t, yaml_mark_t, yaml_node_item_t, yaml_node_pair_t,
    yaml_node_t, yaml_parser_state_t, yaml_parser_t, yaml_read_handler_t, yaml_scalar_style_t,
    yaml_sequence_style_t, yaml_simple_key_t, yaml_stack_t, yaml_tag_directive_t, yaml_token_t,
    yaml_version_directive_t, yaml_write_handler_t, PointerExt, YAML_ANY_ENCODING,
};
use core::mem::{size_of, MaybeUninit};
use core::ptr::{self, addr_of_mut};

const INPUT_RAW_BUFFER_SIZE: usize = 16384;
const INPUT_BUFFER_SIZE: usize = INPUT_RAW_BUFFER_SIZE * 3;
const OUTPUT_BUFFER_SIZE: usize = 16384;
const OUTPUT_RAW_BUFFER_SIZE: usize = OUTPUT_BUFFER_SIZE * 2 + 2;

pub(crate) unsafe fn yaml_malloc(size: size_t) -> *mut libc::c_void {
    malloc(size)
}

pub(crate) unsafe fn yaml_realloc(ptr: *mut libc::c_void, size: size_t) -> *mut libc::c_void {
    if !ptr.is_null() {
        realloc(ptr, size)
    } else {
        malloc(size)
    }
}

pub(crate) unsafe fn yaml_free(ptr: *mut libc::c_void) {
    if !ptr.is_null() {
        free(ptr);
    }
}

pub(crate) unsafe fn yaml_strdup(str: *const yaml_char_t) -> *mut yaml_char_t {
    if str.is_null() {
        return ptr::null_mut::<yaml_char_t>();
    }
    strdup(str as *mut libc::c_char) as *mut yaml_char_t
}

pub(crate) unsafe fn yaml_string_extend(
    start: *mut *mut yaml_char_t,
    pointer: *mut *mut yaml_char_t,
    end: *mut *mut yaml_char_t,
) {
    let new_start: *mut yaml_char_t = yaml_realloc(
        *start as *mut libc::c_void,
        (((*end).c_offset_from(*start) as libc::c_long).force_mul(2_i64)) as size_t,
    ) as *mut yaml_char_t;
    memset(
        new_start.wrapping_offset((*end).c_offset_from(*start) as libc::c_long as isize)
            as *mut libc::c_void,
        0,
        (*end).c_offset_from(*start) as libc::c_ulong,
    );
    *pointer = new_start.wrapping_offset((*pointer).c_offset_from(*start) as libc::c_long as isize);
    *end = new_start.wrapping_offset(
        (((*end).c_offset_from(*start) as libc::c_long).force_mul(2_i64)) as isize,
    );
    *start = new_start;
}

pub(crate) unsafe fn yaml_string_join(
    a_start: *mut *mut yaml_char_t,
    a_pointer: *mut *mut yaml_char_t,
    a_end: *mut *mut yaml_char_t,
    b_start: *mut *mut yaml_char_t,
    b_pointer: *mut *mut yaml_char_t,
    _b_end: *mut *mut yaml_char_t,
) {
    if *b_start == *b_pointer {
        return;
    }
    while (*a_end).c_offset_from(*a_pointer) as libc::c_long
        <= (*b_pointer).c_offset_from(*b_start) as libc::c_long
    {
        yaml_string_extend(a_start, a_pointer, a_end);
    }
    memcpy(
        *a_pointer as *mut libc::c_void,
        *b_start as *const libc::c_void,
        (*b_pointer).c_offset_from(*b_start) as libc::c_ulong,
    );
    *a_pointer =
        (*a_pointer).wrapping_offset((*b_pointer).c_offset_from(*b_start) as libc::c_long as isize);
}

pub(crate) unsafe fn yaml_stack_extend(
    start: *mut *mut libc::c_void,
    top: *mut *mut libc::c_void,
    end: *mut *mut libc::c_void,
) {
    let new_start: *mut libc::c_void = yaml_realloc(
        *start,
        (((*end as *mut libc::c_char).c_offset_from(*start as *mut libc::c_char) as libc::c_long)
            .force_mul(2_i64)) as size_t,
    );
    *top = (new_start as *mut libc::c_char).wrapping_offset(
        (*top as *mut libc::c_char).c_offset_from(*start as *mut libc::c_char) as libc::c_long
            as isize,
    ) as *mut libc::c_void;
    *end = (new_start as *mut libc::c_char).wrapping_offset(
        (((*end as *mut libc::c_char).c_offset_from(*start as *mut libc::c_char) as libc::c_long)
            .force_mul(2_i64)) as isize,
    ) as *mut libc::c_void;
    *start = new_start;
}

pub(crate) unsafe fn yaml_queue_extend(
    start: *mut *mut libc::c_void,
    head: *mut *mut libc::c_void,
    tail: *mut *mut libc::c_void,
    end: *mut *mut libc::c_void,
) {
    if *start == *head && *tail == *end {
        let new_start: *mut libc::c_void = yaml_realloc(
            *start,
            (((*end as *mut libc::c_char).c_offset_from(*start as *mut libc::c_char)
                as libc::c_long)
                .force_mul(2_i64)) as size_t,
        );
        *head = (new_start as *mut libc::c_char).wrapping_offset(
            (*head as *mut libc::c_char).c_offset_from(*start as *mut libc::c_char) as libc::c_long
                as isize,
        ) as *mut libc::c_void;
        *tail = (new_start as *mut libc::c_char).wrapping_offset(
            (*tail as *mut libc::c_char).c_offset_from(*start as *mut libc::c_char) as libc::c_long
                as isize,
        ) as *mut libc::c_void;
        *end = (new_start as *mut libc::c_char).wrapping_offset(
            (((*end as *mut libc::c_char).c_offset_from(*start as *mut libc::c_char)
                as libc::c_long)
                .force_mul(2_i64)) as isize,
        ) as *mut libc::c_void;
        *start = new_start;
    }
    if *tail == *end {
        if *head != *tail {
            memmove(
                *start,
                *head,
                (*tail as *mut libc::c_char).c_offset_from(*head as *mut libc::c_char)
                    as libc::c_ulong,
            );
        }
        *tail = (*start as *mut libc::c_char).wrapping_offset(
            (*tail as *mut libc::c_char).c_offset_from(*head as *mut libc::c_char) as libc::c_long
                as isize,
        ) as *mut libc::c_void;
        *head = *start;
    }
}

/// Initialize a parser.
///
/// This function creates a new parser object. An application is responsible
/// for destroying the object using the yaml_parser_delete() function.
pub unsafe fn yaml_parser_initialize(parser: *mut yaml_parser_t) -> Result<(), ()> {
    __assert!(!parser.is_null());
    *parser = core::mem::MaybeUninit::zeroed().assume_init();
    let parser = &mut *parser;
    BUFFER_INIT!(parser.raw_buffer, INPUT_RAW_BUFFER_SIZE);
    BUFFER_INIT!(parser.buffer, INPUT_BUFFER_SIZE);
    QUEUE_INIT!(parser.tokens, yaml_token_t);
    STACK_INIT!(parser.indents, libc::c_int);
    STACK_INIT!(parser.simple_keys, yaml_simple_key_t);
    STACK_INIT!(parser.states, yaml_parser_state_t);
    STACK_INIT!(parser.marks, yaml_mark_t);
    STACK_INIT!(parser.tag_directives, yaml_tag_directive_t);
    Ok(())
}

/// Destroy a parser.
pub unsafe fn yaml_parser_delete(parser: &mut yaml_parser_t) {
    BUFFER_DEL!(parser.raw_buffer);
    BUFFER_DEL!(parser.buffer);
    while !QUEUE_EMPTY!(parser.tokens) {
        yaml_token_delete(&mut DEQUEUE!(parser.tokens));
    }
    QUEUE_DEL!(parser.tokens);
    STACK_DEL!(parser.indents);
    STACK_DEL!(parser.simple_keys);
    STACK_DEL!(parser.states);
    STACK_DEL!(parser.marks);
    while !STACK_EMPTY!(parser.tag_directives) {
        let tag_directive = POP!(parser.tag_directives);
        yaml_free(tag_directive.handle as *mut libc::c_void);
        yaml_free(tag_directive.prefix as *mut libc::c_void);
    }
    STACK_DEL!(parser.tag_directives);
    *parser = core::mem::MaybeUninit::zeroed().assume_init();
}

unsafe fn yaml_string_read_handler(
    data: *mut libc::c_void,
    buffer: *mut libc::c_uchar,
    mut size: size_t,
    size_read: *mut size_t,
) -> libc::c_int {
    let parser: &mut yaml_parser_t = &mut *(data as *mut yaml_parser_t);
    if parser.input.current == parser.input.end {
        *size_read = 0_u64;
        return 1;
    }
    if size > (*parser).input.end.c_offset_from(parser.input.current) as size_t {
        size = (*parser).input.end.c_offset_from(parser.input.current) as size_t;
    }
    memcpy(
        buffer as *mut libc::c_void,
        parser.input.current as *const libc::c_void,
        size,
    );
    parser.input.current = parser.input.current.wrapping_offset(size as isize);
    *size_read = size;
    1
}

/// Set a string input.
///
/// Note that the `input` pointer must be valid while the `parser` object
/// exists. The application is responsible for destroying `input` after
/// destroying the `parser`.
pub unsafe fn yaml_parser_set_input_string(
    parser: &mut yaml_parser_t,
    input: *const libc::c_uchar,
    size: size_t,
) {
    __assert!((parser.read_handler).is_none());
    __assert!(!input.is_null());
    parser.read_handler = Some(yaml_string_read_handler);
    let parser_ptr = parser as *mut _ as *mut libc::c_void;
    parser.read_handler_data = parser_ptr;
    parser.input.start = input;
    parser.input.current = input;
    parser.input.end = input.wrapping_offset(size as isize);
}

/// Set a generic input handler.
pub unsafe fn yaml_parser_set_input(
    parser: &mut yaml_parser_t,
    handler: yaml_read_handler_t,
    data: *mut libc::c_void,
) {
    __assert!((parser.read_handler).is_none());
    parser.read_handler = Some(handler);
    parser.read_handler_data = data;
}

/// Set the source encoding.
pub unsafe fn yaml_parser_set_encoding(parser: &mut yaml_parser_t, encoding: yaml_encoding_t) {
    __assert!(parser.encoding == YAML_ANY_ENCODING);
    parser.encoding = encoding;
}

/// Initialize an emitter.
///
/// This function creates a new emitter object. An application is responsible
/// for destroying the object using the yaml_emitter_delete() function.
pub unsafe fn yaml_emitter_initialize(emitter: *mut yaml_emitter_t) -> Result<(), ()> {
    __assert!(!emitter.is_null());
    *emitter = core::mem::MaybeUninit::zeroed().assume_init();
    let emitter = &mut *emitter;
    BUFFER_INIT!(emitter.buffer, OUTPUT_BUFFER_SIZE);
    BUFFER_INIT!(emitter.raw_buffer, OUTPUT_RAW_BUFFER_SIZE);
    STACK_INIT!(emitter.states, yaml_emitter_state_t);
    QUEUE_INIT!(emitter.events, yaml_event_t);
    STACK_INIT!(emitter.indents, libc::c_int);
    STACK_INIT!(emitter.tag_directives, yaml_tag_directive_t);
    Ok(())
}

/// Destroy an emitter.
pub unsafe fn yaml_emitter_delete(emitter: &mut yaml_emitter_t) {
    BUFFER_DEL!(emitter.buffer);
    BUFFER_DEL!(emitter.raw_buffer);
    STACK_DEL!(emitter.states);
    while !QUEUE_EMPTY!(emitter.events) {
        let mut event = DEQUEUE!(emitter.events);
        yaml_event_delete(&mut event);
    }
    QUEUE_DEL!(emitter.events);
    STACK_DEL!(emitter.indents);
    while !STACK_EMPTY!(emitter.tag_directives) {
        let tag_directive = POP!(emitter.tag_directives);
        yaml_free(tag_directive.handle as *mut libc::c_void);
        yaml_free(tag_directive.prefix as *mut libc::c_void);
    }
    STACK_DEL!(emitter.tag_directives);
    yaml_free(emitter.anchors as *mut libc::c_void);
    *emitter = core::mem::MaybeUninit::zeroed().assume_init();
}

unsafe fn yaml_string_write_handler(
    data: *mut libc::c_void,
    buffer: *mut libc::c_uchar,
    size: size_t,
) -> libc::c_int {
    let emitter = &mut *(data as *mut yaml_emitter_t);
    if emitter
        .output
        .size
        .wrapping_sub(*emitter.output.size_written)
        < size
    {
        memcpy(
            (*emitter)
                .output
                .buffer
                .wrapping_offset(*emitter.output.size_written as isize)
                as *mut libc::c_void,
            buffer as *const libc::c_void,
            (*emitter)
                .output
                .size
                .wrapping_sub(*emitter.output.size_written),
        );
        *emitter.output.size_written = emitter.output.size;
        return 0;
    }
    memcpy(
        (*emitter)
            .output
            .buffer
            .wrapping_offset(*emitter.output.size_written as isize) as *mut libc::c_void,
        buffer as *const libc::c_void,
        size,
    );
    let fresh153 = &mut (*emitter.output.size_written);
    *fresh153 = (*fresh153 as libc::c_ulong).force_add(size) as size_t;
    1
}

/// Set a string output.
///
/// The emitter will write the output characters to the `output` buffer of the
/// size `size`. The emitter will set `size_written` to the number of written
/// bytes. If the buffer is smaller than required, the emitter produces the
/// YAML_WRITE_ERROR error.
pub unsafe fn yaml_emitter_set_output_string(
    emitter: &mut yaml_emitter_t,
    output: *mut libc::c_uchar,
    size: size_t,
    size_written: *mut size_t,
) {
    __assert!((emitter.write_handler).is_none());
    __assert!(!output.is_null());
    emitter.write_handler = Some(
        yaml_string_write_handler
            as unsafe fn(*mut libc::c_void, *mut libc::c_uchar, size_t) -> libc::c_int,
    );
    emitter.write_handler_data = emitter as *mut _ as *mut libc::c_void;
    emitter.output.buffer = output;
    emitter.output.size = size;
    emitter.output.size_written = size_written;
    *size_written = 0_u64;
}

/// Set a generic output handler.
pub unsafe fn yaml_emitter_set_output(
    emitter: &mut yaml_emitter_t,
    handler: yaml_write_handler_t,
    data: *mut libc::c_void,
) {
    __assert!(emitter.write_handler.is_none());
    emitter.write_handler = Some(handler);
    emitter.write_handler_data = data;
}

/// Set the output encoding.
pub unsafe fn yaml_emitter_set_encoding(emitter: &mut yaml_emitter_t, encoding: yaml_encoding_t) {
    __assert!(emitter.encoding == YAML_ANY_ENCODING);
    emitter.encoding = encoding;
}

/// Set if the output should be in the "canonical" format as in the YAML
/// specification.
pub unsafe fn yaml_emitter_set_canonical(emitter: &mut yaml_emitter_t, canonical: bool) {
    emitter.canonical = canonical;
}

/// Set the indentation increment.
pub unsafe fn yaml_emitter_set_indent(emitter: &mut yaml_emitter_t, indent: libc::c_int) {
    emitter.best_indent = if 1 < indent && indent < 10 { indent } else { 2 };
}

/// Set the preferred line width. -1 means unlimited.
pub unsafe fn yaml_emitter_set_width(emitter: &mut yaml_emitter_t, width: libc::c_int) {
    emitter.best_width = if width >= 0 { width } else { -1 };
}

/// Set if unescaped non-ASCII characters are allowed.
pub unsafe fn yaml_emitter_set_unicode(emitter: &mut yaml_emitter_t, unicode: bool) {
    emitter.unicode = unicode;
}

/// Set the preferred line break.
pub unsafe fn yaml_emitter_set_break(emitter: &mut yaml_emitter_t, line_break: yaml_break_t) {
    emitter.line_break = line_break;
}

/// Free any memory allocated for a token object.
pub unsafe fn yaml_token_delete(token: &mut yaml_token_t) {
    match &token.data {
        YamlTokenData::TagDirective { handle, prefix } => {
            yaml_free(*handle as *mut libc::c_void);
            yaml_free(*prefix as *mut libc::c_void);
        }
        YamlTokenData::Alias { value } => {
            yaml_free(*value as *mut libc::c_void);
        }
        YamlTokenData::Anchor { value } => {
            yaml_free(*value as *mut libc::c_void);
        }
        YamlTokenData::Tag { handle, suffix } => {
            yaml_free(*handle as *mut libc::c_void);
            yaml_free(*suffix as *mut libc::c_void);
        }
        YamlTokenData::Scalar { value, .. } => {
            yaml_free(*value as *mut libc::c_void);
        }
        _ => {}
    }
    *token = yaml_token_t::default();
}

unsafe fn yaml_check_utf8(start: *const yaml_char_t, length: size_t) -> Result<(), ()> {
    let end: *const yaml_char_t = start.wrapping_offset(length as isize);
    let mut pointer: *const yaml_char_t = start;
    while pointer < end {
        let mut octet: libc::c_uchar;
        let mut value: libc::c_uint;
        let mut k: size_t;
        octet = *pointer;
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
        if width == 0 {
            return Err(());
        }
        if pointer.wrapping_offset(width as isize) > end {
            return Err(());
        }
        k = 1_u64;
        while k < width as libc::c_ulong {
            octet = *pointer.wrapping_offset(k as isize);
            if octet & 0xC0 != 0x80 {
                return Err(());
            }
            value = (value << 6).force_add((octet & 0x3F) as libc::c_uint);
            k = k.force_add(1);
        }
        if !(width == 1
            || width == 2 && value >= 0x80
            || width == 3 && value >= 0x800
            || width == 4 && value >= 0x10000)
        {
            return Err(());
        }
        pointer = pointer.wrapping_offset(width as isize);
    }
    Ok(())
}

/// Create the STREAM-START event.
pub unsafe fn yaml_stream_start_event_initialize(
    event: &mut yaml_event_t,
    encoding: yaml_encoding_t,
) -> Result<(), ()> {
    *event = yaml_event_t {
        data: YamlEventData::StreamStart { encoding },
        ..Default::default()
    };
    Ok(())
}

/// Create the STREAM-END event.
pub unsafe fn yaml_stream_end_event_initialize(event: &mut yaml_event_t) -> Result<(), ()> {
    *event = yaml_event_t {
        data: YamlEventData::StreamEnd,
        ..Default::default()
    };
    Ok(())
}

/// Create the DOCUMENT-START event.
///
/// The `implicit` argument is considered as a stylistic parameter and may be
/// ignored by the emitter.
pub unsafe fn yaml_document_start_event_initialize(
    event: &mut yaml_event_t,
    version_directive: *mut yaml_version_directive_t,
    tag_directives_start: *mut yaml_tag_directive_t,
    tag_directives_end: *mut yaml_tag_directive_t,
    implicit: bool,
) -> Result<(), ()> {
    let current_block: u64;
    let mark = yaml_mark_t {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };
    let mut version_directive_copy: *mut yaml_version_directive_t =
        ptr::null_mut::<yaml_version_directive_t>();
    struct TagDirectivesCopy {
        start: *mut yaml_tag_directive_t,
        end: *mut yaml_tag_directive_t,
        top: *mut yaml_tag_directive_t,
    }
    let mut tag_directives_copy = TagDirectivesCopy {
        start: ptr::null_mut::<yaml_tag_directive_t>(),
        end: ptr::null_mut::<yaml_tag_directive_t>(),
        top: ptr::null_mut::<yaml_tag_directive_t>(),
    };
    let mut value = yaml_tag_directive_t {
        handle: ptr::null_mut::<yaml_char_t>(),
        prefix: ptr::null_mut::<yaml_char_t>(),
    };
    __assert!(
        !tag_directives_start.is_null() && !tag_directives_end.is_null()
            || tag_directives_start == tag_directives_end
    );
    if !version_directive.is_null() {
        version_directive_copy = yaml_malloc(size_of::<yaml_version_directive_t>() as libc::c_ulong)
            as *mut yaml_version_directive_t;
        (*version_directive_copy).major = (*version_directive).major;
        (*version_directive_copy).minor = (*version_directive).minor;
    }
    if tag_directives_start != tag_directives_end {
        let mut tag_directive: *mut yaml_tag_directive_t;
        STACK_INIT!(tag_directives_copy, yaml_tag_directive_t);
        tag_directive = tag_directives_start;
        loop {
            if !(tag_directive != tag_directives_end) {
                current_block = 16203760046146113240;
                break;
            }
            __assert!(!((*tag_directive).handle).is_null());
            __assert!(!((*tag_directive).prefix).is_null());
            if yaml_check_utf8(
                (&*tag_directive).handle,
                strlen((&*tag_directive).handle as *mut libc::c_char),
            )
            .is_err()
            {
                current_block = 14964981520188694172;
                break;
            }
            if yaml_check_utf8(
                (*tag_directive).prefix,
                strlen((*tag_directive).prefix as *mut libc::c_char),
            )
            .is_err()
            {
                current_block = 14964981520188694172;
                break;
            }
            value.handle = yaml_strdup((&*tag_directive).handle);
            value.prefix = yaml_strdup((&*tag_directive).prefix);
            if value.handle.is_null() || value.prefix.is_null() {
                current_block = 14964981520188694172;
                break;
            }
            PUSH!(tag_directives_copy, value);
            value = yaml_tag_directive_t {
                handle: ptr::null_mut::<yaml_char_t>(),
                prefix: ptr::null_mut::<yaml_char_t>(),
            };
            tag_directive = tag_directive.wrapping_offset(1);
        }
    } else {
        current_block = 16203760046146113240;
    }
    if current_block != 14964981520188694172 {
        *event = yaml_event_t::default();
        event.start_mark = mark;
        event.end_mark = mark;
        event.data = YamlEventData::DocumentStart {
            version_directive: version_directive_copy,
            tag_directives_start: tag_directives_copy.start,
            tag_directives_end: tag_directives_copy.end,
            implicit,
        };
        return Ok(());
    }
    yaml_free(version_directive_copy as *mut libc::c_void);
    while !STACK_EMPTY!(tag_directives_copy) {
        let value = POP!(tag_directives_copy);
        yaml_free(value.handle as *mut libc::c_void);
        yaml_free(value.prefix as *mut libc::c_void);
    }
    STACK_DEL!(tag_directives_copy);
    yaml_free(value.handle as *mut libc::c_void);
    yaml_free(value.prefix as *mut libc::c_void);
    Err(())
}

/// Create the DOCUMENT-END event.
///
/// The `implicit` argument is considered as a stylistic parameter and may be
/// ignored by the emitter.
pub unsafe fn yaml_document_end_event_initialize(
    event: &mut yaml_event_t,
    implicit: bool,
) -> Result<(), ()> {
    *event = yaml_event_t {
        data: YamlEventData::DocumentEnd { implicit },
        ..Default::default()
    };
    Ok(())
}

/// Create an ALIAS event.
pub unsafe fn yaml_alias_event_initialize(
    event: &mut yaml_event_t,
    anchor: *const yaml_char_t,
) -> Result<(), ()> {
    __assert!(!anchor.is_null());
    yaml_check_utf8(anchor, strlen(anchor as *mut libc::c_char))?;
    let anchor_copy: *mut yaml_char_t = yaml_strdup(anchor);
    if anchor_copy.is_null() {
        return Err(());
    }
    *event = yaml_event_t {
        data: YamlEventData::Alias {
            anchor: anchor_copy,
        },
        ..Default::default()
    };
    Ok(())
}

/// Create a SCALAR event.
///
/// The `style` argument may be ignored by the emitter.
///
/// Either the `tag` attribute or one of the `plain_implicit` and
/// `quoted_implicit` flags must be set.
///
pub unsafe fn yaml_scalar_event_initialize(
    event: &mut yaml_event_t,
    anchor: *const yaml_char_t,
    tag: *const yaml_char_t,
    value: *const yaml_char_t,
    mut length: libc::c_int,
    plain_implicit: bool,
    quoted_implicit: bool,
    style: yaml_scalar_style_t,
) -> Result<(), ()> {
    let mut current_block: u64;
    let mark = yaml_mark_t {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };
    let mut anchor_copy: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    let mut tag_copy: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    let mut value_copy: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    __assert!(!value.is_null());
    if !anchor.is_null() {
        if yaml_check_utf8(anchor, strlen(anchor as *mut libc::c_char)).is_err() {
            current_block = 16285396129609901221;
        } else {
            anchor_copy = yaml_strdup(anchor);
            if anchor_copy.is_null() {
                current_block = 16285396129609901221;
            } else {
                current_block = 8515828400728868193;
            }
        }
    } else {
        current_block = 8515828400728868193;
    }
    if current_block == 8515828400728868193 {
        if !tag.is_null() {
            if yaml_check_utf8(tag, strlen(tag as *mut libc::c_char)).is_err() {
                current_block = 16285396129609901221;
            } else {
                tag_copy = yaml_strdup(tag);
                if tag_copy.is_null() {
                    current_block = 16285396129609901221;
                } else {
                    current_block = 12800627514080957624;
                }
            }
        } else {
            current_block = 12800627514080957624;
        }
        if current_block != 16285396129609901221 {
            if length < 0 {
                length = strlen(value as *mut libc::c_char) as libc::c_int;
            }
            if let Ok(()) = yaml_check_utf8(value, length as size_t) {
                value_copy = yaml_malloc(length.force_add(1) as size_t) as *mut yaml_char_t;
                memcpy(
                    value_copy as *mut libc::c_void,
                    value as *const libc::c_void,
                    length as libc::c_ulong,
                );
                *value_copy.wrapping_offset(length as isize) = b'\0';
                *event = yaml_event_t {
                    data: YamlEventData::Scalar {
                        anchor: anchor_copy,
                        tag: tag_copy,
                        value: value_copy,
                        length: length as _,
                        plain_implicit,
                        quoted_implicit,
                        style,
                    },
                    start_mark: mark,
                    end_mark: mark,
                };
                return Ok(());
            }
        }
    }
    yaml_free(anchor_copy as *mut libc::c_void);
    yaml_free(tag_copy as *mut libc::c_void);
    yaml_free(value_copy as *mut libc::c_void);
    Err(())
}

/// Create a SEQUENCE-START event.
///
/// The `style` argument may be ignored by the emitter.
///
/// Either the `tag` attribute or the `implicit` flag must be set.
pub unsafe fn yaml_sequence_start_event_initialize(
    event: &mut yaml_event_t,
    anchor: *const yaml_char_t,
    tag: *const yaml_char_t,
    implicit: bool,
    style: yaml_sequence_style_t,
) -> Result<(), ()> {
    let mut current_block: u64;
    let mark = yaml_mark_t {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };
    let mut anchor_copy: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    let mut tag_copy: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    if !anchor.is_null() {
        if yaml_check_utf8(anchor, strlen(anchor as *mut libc::c_char)).is_err() {
            current_block = 8817775685815971442;
        } else {
            anchor_copy = yaml_strdup(anchor);
            if anchor_copy.is_null() {
                current_block = 8817775685815971442;
            } else {
                current_block = 11006700562992250127;
            }
        }
    } else {
        current_block = 11006700562992250127;
    }
    match current_block {
        11006700562992250127 => {
            if !tag.is_null() {
                if yaml_check_utf8(tag, strlen(tag as *mut libc::c_char)).is_err() {
                    current_block = 8817775685815971442;
                } else {
                    tag_copy = yaml_strdup(tag);
                    if tag_copy.is_null() {
                        current_block = 8817775685815971442;
                    } else {
                        current_block = 7651349459974463963;
                    }
                }
            } else {
                current_block = 7651349459974463963;
            }
            if current_block != 8817775685815971442 {
                *event = yaml_event_t {
                    data: YamlEventData::SequenceStart {
                        anchor: anchor_copy,
                        tag: tag_copy,
                        implicit,
                        style,
                    },
                    start_mark: mark,
                    end_mark: mark,
                };
                return Ok(());
            }
        }
        _ => {}
    }
    yaml_free(anchor_copy as *mut libc::c_void);
    yaml_free(tag_copy as *mut libc::c_void);
    Err(())
}

/// Create a SEQUENCE-END event.
pub unsafe fn yaml_sequence_end_event_initialize(event: &mut yaml_event_t) -> Result<(), ()> {
    *event = yaml_event_t {
        data: YamlEventData::SequenceEnd,
        ..Default::default()
    };
    Ok(())
}

/// Create a MAPPING-START event.
///
/// The `style` argument may be ignored by the emitter.
///
/// Either the `tag` attribute or the `implicit` flag must be set.
pub unsafe fn yaml_mapping_start_event_initialize(
    event: &mut yaml_event_t,
    anchor: *const yaml_char_t,
    tag: *const yaml_char_t,
    implicit: bool,
    style: yaml_mapping_style_t,
) -> Result<(), ()> {
    let mut current_block: u64;
    let mark = yaml_mark_t {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };
    let mut anchor_copy: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    let mut tag_copy: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    if !anchor.is_null() {
        if yaml_check_utf8(anchor, strlen(anchor as *mut libc::c_char)).is_err() {
            current_block = 14748279734549812740;
        } else {
            anchor_copy = yaml_strdup(anchor);
            if anchor_copy.is_null() {
                current_block = 14748279734549812740;
            } else {
                current_block = 11006700562992250127;
            }
        }
    } else {
        current_block = 11006700562992250127;
    }
    if current_block == 11006700562992250127 {
        if !tag.is_null() {
            if yaml_check_utf8(tag, strlen(tag as *mut libc::c_char)).is_err() {
                current_block = 14748279734549812740;
            } else {
                tag_copy = yaml_strdup(tag);
                if tag_copy.is_null() {
                    current_block = 14748279734549812740;
                } else {
                    current_block = 7651349459974463963;
                }
            }
        } else {
            current_block = 7651349459974463963;
        }
        if current_block != 14748279734549812740 {
            *event = yaml_event_t {
                data: YamlEventData::MappingStart {
                    anchor: anchor_copy,
                    tag: tag_copy,
                    implicit,
                    style,
                },
                start_mark: mark,
                end_mark: mark,
            };
            return Ok(());
        }
    }
    yaml_free(anchor_copy as *mut libc::c_void);
    yaml_free(tag_copy as *mut libc::c_void);
    Err(())
}

/// Create a MAPPING-END event.
pub unsafe fn yaml_mapping_end_event_initialize(event: &mut yaml_event_t) -> Result<(), ()> {
    *event = yaml_event_t {
        data: YamlEventData::MappingEnd,
        ..Default::default()
    };
    Ok(())
}

/// Free any memory allocated for an event object.
pub unsafe fn yaml_event_delete(event: &mut yaml_event_t) {
    let event = core::mem::replace(event, Default::default());

    let mut tag_directive: *mut yaml_tag_directive_t;

    match event.data {
        YamlEventData::NoEvent => (),
        YamlEventData::StreamStart { .. } => (),
        YamlEventData::StreamEnd => (),
        YamlEventData::DocumentStart {
            version_directive,
            tag_directives_start,
            tag_directives_end,
            implicit: _,
        } => {
            yaml_free(version_directive as *mut libc::c_void);
            tag_directive = tag_directives_start;
            while tag_directive != tag_directives_end {
                yaml_free((*tag_directive).handle as *mut libc::c_void);
                yaml_free((*tag_directive).prefix as *mut libc::c_void);
                tag_directive = tag_directive.wrapping_offset(1);
            }
            yaml_free(tag_directives_start as *mut libc::c_void);
        }
        YamlEventData::DocumentEnd { .. } => (),
        YamlEventData::Alias { anchor } => {
            yaml_free(anchor as *mut libc::c_void);
        }
        YamlEventData::Scalar {
            anchor, tag, value, ..
        } => {
            yaml_free(anchor as *mut libc::c_void);
            yaml_free(tag as *mut libc::c_void);
            yaml_free(value as *mut libc::c_void);
        }
        YamlEventData::SequenceStart { anchor, tag, .. } => {
            yaml_free(anchor as *mut libc::c_void);
            yaml_free(tag as *mut libc::c_void);
        }
        YamlEventData::SequenceEnd => (),
        YamlEventData::MappingStart { anchor, tag, .. } => {
            yaml_free(anchor as *mut libc::c_void);
            yaml_free(tag as *mut libc::c_void);
        }
        YamlEventData::MappingEnd => (),
    }
}

/// Create a YAML document.
pub unsafe fn yaml_document_initialize(
    document: &mut yaml_document_t,
    version_directive: *mut yaml_version_directive_t,
    tag_directives_start: *mut yaml_tag_directive_t,
    tag_directives_end: *mut yaml_tag_directive_t,
    start_implicit: bool,
    end_implicit: bool,
) -> Result<(), ()> {
    let current_block: u64;
    struct Nodes {
        start: *mut yaml_node_t,
        end: *mut yaml_node_t,
        top: *mut yaml_node_t,
    }
    let mut nodes = Nodes {
        start: ptr::null_mut::<yaml_node_t>(),
        end: ptr::null_mut::<yaml_node_t>(),
        top: ptr::null_mut::<yaml_node_t>(),
    };
    let mut version_directive_copy: *mut yaml_version_directive_t =
        ptr::null_mut::<yaml_version_directive_t>();
    struct TagDirectivesCopy {
        start: *mut yaml_tag_directive_t,
        end: *mut yaml_tag_directive_t,
        top: *mut yaml_tag_directive_t,
    }
    let mut tag_directives_copy = TagDirectivesCopy {
        start: ptr::null_mut::<yaml_tag_directive_t>(),
        end: ptr::null_mut::<yaml_tag_directive_t>(),
        top: ptr::null_mut::<yaml_tag_directive_t>(),
    };
    let mut value = yaml_tag_directive_t {
        handle: ptr::null_mut::<yaml_char_t>(),
        prefix: ptr::null_mut::<yaml_char_t>(),
    };
    let mark = yaml_mark_t {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };
    __assert!(
        !tag_directives_start.is_null() && !tag_directives_end.is_null()
            || tag_directives_start == tag_directives_end
    );
    STACK_INIT!(nodes, yaml_node_t);
    if !version_directive.is_null() {
        version_directive_copy = yaml_malloc(size_of::<yaml_version_directive_t>() as libc::c_ulong)
            as *mut yaml_version_directive_t;
        (*version_directive_copy).major = (*version_directive).major;
        (*version_directive_copy).minor = (*version_directive).minor;
    }
    if tag_directives_start != tag_directives_end {
        let mut tag_directive: *mut yaml_tag_directive_t;
        STACK_INIT!(tag_directives_copy, yaml_tag_directive_t);
        tag_directive = tag_directives_start;
        loop {
            if !(tag_directive != tag_directives_end) {
                current_block = 14818589718467733107;
                break;
            }
            __assert!(!((*tag_directive).handle).is_null());
            __assert!(!((*tag_directive).prefix).is_null());
            if yaml_check_utf8(
                (*tag_directive).handle,
                strlen((*tag_directive).handle as *mut libc::c_char),
            )
            .is_err()
            {
                current_block = 8142820162064489797;
                break;
            }
            if yaml_check_utf8(
                (*tag_directive).prefix,
                strlen((*tag_directive).prefix as *mut libc::c_char),
            )
            .is_err()
            {
                current_block = 8142820162064489797;
                break;
            }
            value.handle = yaml_strdup((*tag_directive).handle);
            value.prefix = yaml_strdup((*tag_directive).prefix);
            if value.handle.is_null() || value.prefix.is_null() {
                current_block = 8142820162064489797;
                break;
            }
            PUSH!(tag_directives_copy, value);
            value = yaml_tag_directive_t {
                handle: ptr::null_mut::<yaml_char_t>(),
                prefix: ptr::null_mut::<yaml_char_t>(),
            };
            tag_directive = tag_directive.wrapping_offset(1);
        }
    } else {
        current_block = 14818589718467733107;
    }
    if current_block != 8142820162064489797 {
        *document = core::mem::MaybeUninit::zeroed().assume_init();
        document.nodes.start = nodes.start;
        document.nodes.end = nodes.end;
        document.nodes.top = nodes.start;
        document.version_directive = version_directive_copy;
        document.tag_directives.start = tag_directives_copy.start;
        document.tag_directives.end = tag_directives_copy.top;
        document.start_implicit = start_implicit;
        document.end_implicit = end_implicit;
        document.start_mark = mark;
        document.end_mark = mark;
        return Ok(());
    }
    STACK_DEL!(nodes);
    yaml_free(version_directive_copy as *mut libc::c_void);
    while !STACK_EMPTY!(tag_directives_copy) {
        let value = POP!(tag_directives_copy);
        yaml_free(value.handle as *mut libc::c_void);
        yaml_free(value.prefix as *mut libc::c_void);
    }
    STACK_DEL!(tag_directives_copy);
    yaml_free(value.handle as *mut libc::c_void);
    yaml_free(value.prefix as *mut libc::c_void);
    Err(())
}

/// Delete a YAML document and all its nodes.
pub unsafe fn yaml_document_delete(document: &mut yaml_document_t) {
    let mut tag_directive: *mut yaml_tag_directive_t;
    while !STACK_EMPTY!(document.nodes) {
        let mut node = POP!(document.nodes);
        yaml_free(node.tag as *mut libc::c_void);
        match node.data {
            YamlNodeData::NoNode => {
                assert!(false);
            }
            YamlNodeData::Scalar { ref value, .. } => {
                yaml_free(*value as *mut libc::c_void);
            }
            YamlNodeData::Sequence { ref mut items, .. } => {
                STACK_DEL!(items);
            }
            YamlNodeData::Mapping { ref mut pairs, .. } => {
                STACK_DEL!(pairs);
            }
        }
    }
    STACK_DEL!(document.nodes);
    yaml_free(document.version_directive as *mut libc::c_void);
    tag_directive = document.tag_directives.start;
    while tag_directive != document.tag_directives.end {
        yaml_free((*tag_directive).handle as *mut libc::c_void);
        yaml_free((*tag_directive).prefix as *mut libc::c_void);
        tag_directive = tag_directive.wrapping_offset(1);
    }
    yaml_free(document.tag_directives.start as *mut libc::c_void);
    *document = MaybeUninit::zeroed().assume_init();
}

/// Get a node of a YAML document.
///
/// The pointer returned by this function is valid until any of the functions
/// modifying the documents are called.
///
/// Returns the node objct or NULL if `node_id` is out of range.
pub unsafe fn yaml_document_get_node(
    document: &mut yaml_document_t,
    index: libc::c_int,
) -> *mut yaml_node_t {
    if index > 0 && document.nodes.start.wrapping_offset(index as isize) <= document.nodes.top {
        return (*document)
            .nodes
            .start
            .wrapping_offset(index as isize)
            .wrapping_offset(-1_isize);
    }
    ptr::null_mut::<yaml_node_t>()
}

/// Get the root of a YAML document node.
///
/// The root object is the first object added to the document.
///
/// The pointer returned by this function is valid until any of the functions
/// modifying the documents are called.
///
/// An empty document produced by the parser signifies the end of a YAML stream.
///
/// Returns the node object or NULL if the document is empty.
pub unsafe fn yaml_document_get_root_node(document: &mut yaml_document_t) -> *mut yaml_node_t {
    if document.nodes.top != document.nodes.start {
        return document.nodes.start;
    }
    ptr::null_mut::<yaml_node_t>()
}

/// Create a SCALAR node and attach it to the document.
///
/// The `style` argument may be ignored by the emitter.
///
/// Returns the node id or 0 on error.
#[must_use]
pub unsafe fn yaml_document_add_scalar(
    document: &mut yaml_document_t,
    mut tag: *const yaml_char_t,
    value: *const yaml_char_t,
    mut length: libc::c_int,
    style: yaml_scalar_style_t,
) -> libc::c_int {
    let mark = yaml_mark_t {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };
    let mut tag_copy: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    let mut value_copy: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    __assert!(!value.is_null());
    if tag.is_null() {
        tag = b"tag:yaml.org,2002:str\0" as *const u8 as *const libc::c_char as *mut yaml_char_t;
    }
    if let Ok(()) = yaml_check_utf8(tag, strlen(tag as *mut libc::c_char)) {
        tag_copy = yaml_strdup(tag);
        if !tag_copy.is_null() {
            if length < 0 {
                length = strlen(value as *mut libc::c_char) as libc::c_int;
            }
            if let Ok(()) = yaml_check_utf8(value, length as size_t) {
                value_copy = yaml_malloc(length.force_add(1) as size_t) as *mut yaml_char_t;
                memcpy(
                    value_copy as *mut libc::c_void,
                    value as *const libc::c_void,
                    length as libc::c_ulong,
                );
                *value_copy.wrapping_offset(length as isize) = b'\0';
                let node = yaml_node_t {
                    data: YamlNodeData::Scalar {
                        value: value_copy,
                        length: length as size_t,
                        style: style,
                    },
                    tag: tag_copy,
                    start_mark: mark,
                    end_mark: mark,
                };
                PUSH!(document.nodes, node);
                return document.nodes.top.c_offset_from(document.nodes.start) as libc::c_int;
            }
        }
    }
    yaml_free(tag_copy as *mut libc::c_void);
    yaml_free(value_copy as *mut libc::c_void);
    0
}

/// Create a SEQUENCE node and attach it to the document.
///
/// The `style` argument may be ignored by the emitter.
///
/// Returns the node id or 0 on error.
#[must_use]
pub unsafe fn yaml_document_add_sequence(
    document: &mut yaml_document_t,
    mut tag: *const yaml_char_t,
    style: yaml_sequence_style_t,
) -> libc::c_int {
    let mark = yaml_mark_t {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };
    let mut tag_copy: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    struct Items {
        start: *mut yaml_node_item_t,
        end: *mut yaml_node_item_t,
        top: *mut yaml_node_item_t,
    }
    let mut items = Items {
        start: ptr::null_mut::<yaml_node_item_t>(),
        end: ptr::null_mut::<yaml_node_item_t>(),
        top: ptr::null_mut::<yaml_node_item_t>(),
    };
    if tag.is_null() {
        tag = b"tag:yaml.org,2002:seq\0" as *const u8 as *const libc::c_char as *mut yaml_char_t;
    }
    if let Ok(()) = yaml_check_utf8(tag, strlen(tag as *mut libc::c_char)) {
        tag_copy = yaml_strdup(tag);
        if !tag_copy.is_null() {
            STACK_INIT!(items, yaml_node_item_t);
            let node = yaml_node_t {
                data: YamlNodeData::Sequence {
                    items: yaml_stack_t {
                        start: items.start,
                        end: items.end,
                        top: items.start,
                    },
                    style,
                },
                tag: tag_copy,
                start_mark: mark,
                end_mark: mark,
            };
            PUSH!(document.nodes, node);
            return document.nodes.top.c_offset_from(document.nodes.start) as libc::c_int;
        }
    }
    STACK_DEL!(items);
    yaml_free(tag_copy as *mut libc::c_void);
    0
}

/// Create a MAPPING node and attach it to the document.
///
/// The `style` argument may be ignored by the emitter.
///
/// Returns the node id or 0 on error.
#[must_use]
pub unsafe fn yaml_document_add_mapping(
    document: &mut yaml_document_t,
    mut tag: *const yaml_char_t,
    style: yaml_mapping_style_t,
) -> libc::c_int {
    let mark = yaml_mark_t {
        index: 0_u64,
        line: 0_u64,
        column: 0_u64,
    };
    let mut tag_copy: *mut yaml_char_t = ptr::null_mut::<yaml_char_t>();
    struct Pairs {
        start: *mut yaml_node_pair_t,
        end: *mut yaml_node_pair_t,
        top: *mut yaml_node_pair_t,
    }
    let mut pairs = Pairs {
        start: ptr::null_mut::<yaml_node_pair_t>(),
        end: ptr::null_mut::<yaml_node_pair_t>(),
        top: ptr::null_mut::<yaml_node_pair_t>(),
    };
    if tag.is_null() {
        tag = b"tag:yaml.org,2002:map\0" as *const u8 as *const libc::c_char as *mut yaml_char_t;
    }
    if let Ok(()) = yaml_check_utf8(tag, strlen(tag as *mut libc::c_char)) {
        tag_copy = yaml_strdup(tag);
        if !tag_copy.is_null() {
            STACK_INIT!(pairs, yaml_node_pair_t);

            let node = yaml_node_t {
                data: YamlNodeData::Mapping {
                    pairs: yaml_stack_t {
                        start: pairs.start,
                        end: pairs.end,
                        top: pairs.start,
                    },
                    style,
                },
                tag: tag_copy,
                start_mark: mark,
                end_mark: mark,
            };

            PUSH!(document.nodes, node);
            return document.nodes.top.c_offset_from(document.nodes.start) as libc::c_int;
        }
    }
    STACK_DEL!(pairs);
    yaml_free(tag_copy as *mut libc::c_void);
    0
}

/// Add an item to a SEQUENCE node.
pub unsafe fn yaml_document_append_sequence_item(
    document: &mut yaml_document_t,
    sequence: libc::c_int,
    item: libc::c_int,
) -> Result<(), ()> {
    __assert!(
        sequence > 0
            && (document.nodes.start).wrapping_offset(sequence as isize) <= document.nodes.top
    );
    __assert!(matches!(
        (*(document.nodes.start).wrapping_offset((sequence - 1) as isize)).data,
        YamlNodeData::Sequence { .. }
    ));
    __assert!(
        item > 0 && (document.nodes.start).wrapping_offset(item as isize) <= document.nodes.top
    );
    if let YamlNodeData::Sequence { ref mut items, .. } =
        (*(document.nodes.start).wrapping_offset((sequence - 1) as isize)).data
    {
        PUSH!(*items, item);
    }
    Ok(())
}

/// Add a pair of a key and a value to a MAPPING node.
pub unsafe fn yaml_document_append_mapping_pair(
    document: &mut yaml_document_t,
    mapping: libc::c_int,
    key: libc::c_int,
    value: libc::c_int,
) -> Result<(), ()> {
    __assert!(
        mapping > 0
            && (document.nodes.start).wrapping_offset(mapping as isize) <= document.nodes.top
    );
    __assert!(matches!(
        (*(document.nodes.start).wrapping_offset((mapping - 1) as isize)).data,
        YamlNodeData::Mapping { .. }
    ));
    __assert!(
        key > 0 && (document.nodes.start).wrapping_offset(key as isize) <= document.nodes.top
    );
    __assert!(
        value > 0 && (document.nodes.start).wrapping_offset(value as isize) <= document.nodes.top
    );
    let pair = yaml_node_pair_t { key, value };
    if let YamlNodeData::Mapping { ref mut pairs, .. } =
        (*(document.nodes.start).wrapping_offset((mapping - 1) as isize)).data
    {
        PUSH!(*pairs, pair);
    }
    Ok(())
}
