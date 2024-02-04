#![doc = include_str!("../README.md")]
#![doc(html_root_url = "https://docs.rs/libyaml-safer/0.1.0")]
#![allow(non_snake_case)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::fn_params_excessive_bools,
    clippy::manual_range_contains,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::match_wildcard_for_single_variants
)]
#![deny(unsafe_code)]

extern crate alloc;

#[macro_use]
mod macros;

mod document;
mod dumper;
mod emitter;
mod error;
mod event;
mod loader;
mod parser;
mod reader;
mod scanner;
mod token;
mod writer;

pub use crate::document::*;
pub use crate::dumper::{yaml_emitter_close, yaml_emitter_dump, yaml_emitter_open};
pub use crate::emitter::*;
pub use crate::error::*;
pub use crate::event::*;
pub use crate::loader::yaml_parser_load;
pub use crate::parser::*;
pub use crate::scanner::yaml_parser_scan;
pub use crate::token::*;
pub use crate::writer::yaml_emitter_flush;

pub(crate) const INPUT_RAW_BUFFER_SIZE: usize = 16384;
pub(crate) const INPUT_BUFFER_SIZE: usize = INPUT_RAW_BUFFER_SIZE;
pub(crate) const OUTPUT_BUFFER_SIZE: usize = 16384;

/// The tag `!!null` with the only possible value: `null`.
pub const NULL_TAG: &str = "tag:yaml.org,2002:null";
/// The tag `!!bool` with the values: `true` and `false`.
pub const BOOL_TAG: &str = "tag:yaml.org,2002:bool";
/// The tag `!!str` for string values.
pub const STR_TAG: &str = "tag:yaml.org,2002:str";
/// The tag `!!int` for integer values.
pub const INT_TAG: &str = "tag:yaml.org,2002:int";
/// The tag `!!float` for float values.
pub const FLOAT_TAG: &str = "tag:yaml.org,2002:float";
/// The tag `!!timestamp` for date and time values.
pub const TIMESTAMP_TAG: &str = "tag:yaml.org,2002:timestamp";

/// The tag `!!seq` is used to denote sequences.
pub const SEQ_TAG: &str = "tag:yaml.org,2002:seq";
/// The tag `!!map` is used to denote mapping.
pub const MAP_TAG: &str = "tag:yaml.org,2002:map";

/// The default scalar tag is `!!str`.
pub const DEFAULT_SCALAR_TAG: &str = STR_TAG;
/// The default sequence tag is `!!seq`.
pub const DEFAULT_SEQUENCE_TAG: &str = SEQ_TAG;
/// The default mapping tag is `!!map`.
pub const DEFAULT_MAPPING_TAG: &str = MAP_TAG;

/// The version directive data.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct VersionDirective {
    /// The major version number.
    pub major: i32,
    /// The minor version number.
    pub minor: i32,
}

/// The tag directive data.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TagDirective {
    /// The tag handle.
    pub handle: String,
    /// The tag prefix.
    pub prefix: String,
}

/// The stream encoding.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum Encoding {
    /// Let the parser choose the encoding.
    #[default]
    Any = 0,
    /// The default UTF-8 encoding.
    Utf8 = 1,
    /// The UTF-16-LE encoding with BOM.
    Utf16Le = 2,
    /// The UTF-16-BE encoding with BOM.
    Utf16Be = 3,
}

/// Line break type.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum Break {
    /// Let the parser choose the break type.
    #[default]
    Any = 0,
    /// Use CR for line breaks (Mac style).
    Cr = 1,
    /// Use LN for line breaks (Unix style).
    Ln = 2,
    /// Use CR LN for line breaks (DOS style).
    CrLn = 3,
}

/// Scalar styles.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum ScalarStyle {
    /// Let the emitter choose the style.
    #[default]
    Any = 0,
    /// The plain scalar style.
    Plain = 1,
    /// The single-quoted scalar style.
    SingleQuoted = 2,
    /// The double-quoted scalar style.
    DoubleQuoted = 3,
    /// The literal scalar style.
    Literal = 4,
    /// The folded scalar style.
    Folded = 5,
}

/// Sequence styles.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum SequenceStyle {
    /// Let the emitter choose the style.
    Any = 0,
    /// The block sequence style.
    Block = 1,
    /// The flow sequence style.
    Flow = 2,
}

/// Mapping styles.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum MappingStyle {
    /// Let the emitter choose the style.
    Any = 0,
    /// The block mapping style.
    Block = 1,
    /// The flow mapping style.
    Flow = 2,
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn sanity() {
        const SANITY_INPUT: &str = r#"
unicode: "Sosa did fine.\u263A"
control: "\b1998\t1999\t2000\n"
hex esc: "\x0d\x0a is \r\n"

single: '"Howdy!" he cried.'
quoted: ' # Not a ''comment''.'
tie-fighter: '|\-*-/|'
"#;
        let mut parser = yaml_parser_new();
        // const SANITY_INPUT: &'static str =
        //     "Mark McGwire:\n  hr: 65\n  avg: 0.278\nSammy Sosa:\n  hr: 63\n  avg: 0.288\n";
        let mut read_in = SANITY_INPUT.as_bytes();
        yaml_parser_set_input_string(&mut parser, &mut read_in);
        let _doc = yaml_parser_load(&mut parser).unwrap();
        // let mut doc = doc.assume_init();

        // let mut emitter = core::mem::MaybeUninit::uninit();
        // yaml_emitter_initialize(emitter.as_mut_ptr()).unwrap();
        // let mut emitter = emitter.assume_init();

        // let mut output = vec![0u8; 1024];
        // let mut size_written = 0;
        // yaml_emitter_set_output_string(
        //     &mut emitter,
        //     output.as_mut_ptr(),
        //     1024,
        //     &mut size_written,
        // );

        // if yaml_emitter_dump(&mut emitter, &mut doc).is_err() {
        //     panic!("emitter error: {:?} {:?}", emitter.error, emitter.problem);
        // }
        // output.resize(size_written as _, 0);
        // let output_str = core::str::from_utf8(&output).expect("invalid UTF-8");
        // assert_eq!(output_str, SANITY_INPUT);
    }

    const TEST_CASE_QF4Y: &str = r"[
foo: bar
]
";

    #[test]
    fn test_case() {
        let mut parser = yaml_parser_new();
        let mut input = TEST_CASE_QF4Y.as_bytes();
        yaml_parser_set_input_string(&mut parser, &mut input);
        let _doc = yaml_parser_load(&mut parser).unwrap();
    }

    // #[test]
    // fn integration_s7bg() {
    //     unsafe {
    //         let mut emitter = emitter_new();
    //         let mut output = vec![0u8; 1024];
    //         let mut size_written = 0;
    //         yaml_emitter_set_output_string(
    //             &mut emitter,
    //             output.as_mut_ptr(),
    //             1024,
    //             &mut size_written,
    //         );

    //         let mut event = yaml_event_t::default();
    //         yaml_stream_start_event_initialize(&mut event, YAML_UTF8_ENCODING).unwrap();
    //         yaml_emitter_emit(&mut emitter, core::mem::take(&mut event)).unwrap();
    //         yaml_document_start_event_initialize(&mut event, None, &[], true).unwrap();
    //         yaml_emitter_emit(&mut emitter, core::mem::take(&mut event)).unwrap();
    //         yaml_sequence_start_event_initialize(
    //             &mut event,
    //             None,
    //             None,
    //             false,
    //             YAML_BLOCK_SEQUENCE_STYLE,
    //         )
    //         .unwrap();
    //         yaml_emitter_emit(&mut emitter, core::mem::take(&mut event)).unwrap();
    //         yaml_scalar_event_initialize(
    //             &mut event,
    //             None,
    //             None,
    //             ":,",
    //             true,
    //             true,
    //             YAML_PLAIN_SCALAR_STYLE,
    //         )
    //         .unwrap();
    //         yaml_emitter_emit(&mut emitter, core::mem::take(&mut event)).unwrap();
    //         yaml_sequence_end_event_initialize(&mut event).unwrap();
    //         yaml_emitter_emit(&mut emitter, core::mem::take(&mut event)).unwrap();
    //         yaml_document_end_event_initialize(&mut event, true).unwrap();
    //         yaml_emitter_emit(&mut emitter, core::mem::take(&mut event)).unwrap();
    //         yaml_stream_end_event_initialize(&mut event).unwrap();
    //         yaml_emitter_emit(&mut emitter, core::mem::take(&mut event)).unwrap();

    //         assert_eq!(
    //             core::str::from_utf8(&output[0..size_written as usize]).unwrap(),
    //             "- :,\n"
    //         );
    //     }
    // }

    #[test]
    fn integration_hs5t() {
        let mut emitter = yaml_emitter_new();
        let mut output = Vec::new();
        yaml_emitter_set_output_string(&mut emitter, &mut output);

        let event = Event::stream_start(Encoding::Utf8);
        yaml_emitter_emit(&mut emitter, event).unwrap();
        let event = Event::document_start(None, &[], true);
        yaml_emitter_emit(&mut emitter, event).unwrap();
        let event = Event::scalar(
            None,
            None,
            "1st non-empty\n2nd non-empty 3rd non-empty",
            true,
            true,
            ScalarStyle::Plain,
        );
        yaml_emitter_emit(&mut emitter, event).unwrap();
        let event = Event::document_end(true);
        yaml_emitter_emit(&mut emitter, event).unwrap();
        let event = Event::stream_end();
        yaml_emitter_emit(&mut emitter, event).unwrap();

        assert_eq!(
            core::str::from_utf8(&output),
            Ok("'1st non-empty\n\n  2nd non-empty 3rd non-empty'\n")
        );
    }
}
