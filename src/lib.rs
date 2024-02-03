//! [![github]](https://github.com/dtolnay/unsafe-libyaml)&ensp;[![crates-io]](https://crates.io/crates/unsafe-libyaml)&ensp;[![docs-rs]](https://docs.rs/unsafe-libyaml)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs

#![doc(html_root_url = "https://docs.rs/unsafe-libyaml/0.2.10")]
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

extern crate alloc;

#[macro_use]
mod macros;

mod api;
mod dumper;
mod emitter;
mod error;
mod loader;
mod parser;
mod reader;
mod scanner;
mod writer;
mod yaml;

pub use crate::api::{
    yaml_alias_event_new, yaml_document_add_mapping, yaml_document_add_scalar,
    yaml_document_add_sequence, yaml_document_append_mapping_pair,
    yaml_document_append_sequence_item, yaml_document_delete, yaml_document_end_event_new,
    yaml_document_get_node, yaml_document_get_root_node, yaml_document_new,
    yaml_document_start_event_new, yaml_emitter_new, yaml_emitter_reset, yaml_emitter_set_break,
    yaml_emitter_set_canonical, yaml_emitter_set_encoding, yaml_emitter_set_indent,
    yaml_emitter_set_output, yaml_emitter_set_output_string, yaml_emitter_set_unicode,
    yaml_emitter_set_width, yaml_mapping_end_event_new, yaml_mapping_start_event_new,
    yaml_parser_new, yaml_parser_reset, yaml_parser_set_encoding, yaml_parser_set_input,
    yaml_parser_set_input_string, yaml_scalar_event_new, yaml_sequence_end_event_new,
    yaml_sequence_start_event_new, yaml_stream_end_event_new, yaml_stream_start_event_new,
};
pub use crate::dumper::{yaml_emitter_close, yaml_emitter_dump, yaml_emitter_open};
pub use crate::emitter::yaml_emitter_emit;
pub use crate::error::*;
pub use crate::loader::yaml_parser_load;
pub use crate::parser::yaml_parser_parse;
pub use crate::scanner::yaml_parser_scan;
pub use crate::writer::yaml_emitter_flush;
pub use crate::yaml::{
    AliasData, Break, Document, Emitter, EmitterState, Encoding, Event, EventData, MappingStyle,
    Mark, Node, NodeItem, NodePair, Parser, ParserState, ScalarStyle, SequenceStyle, SimpleKey,
    TagDirective, Token, VersionDirective,
};
#[doc(hidden)]
pub use crate::yaml::{
    BOOL_TAG, DEFAULT_MAPPING_TAG, DEFAULT_SCALAR_TAG, DEFAULT_SEQUENCE_TAG, FLOAT_TAG, INT_TAG,
    MAP_TAG, NULL_TAG, SEQ_TAG, STR_TAG, TIMESTAMP_TAG,
};

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

        let event = yaml_stream_start_event_new(Encoding::Utf8);
        yaml_emitter_emit(&mut emitter, event).unwrap();
        let event = yaml_document_start_event_new(None, &[], true);
        yaml_emitter_emit(&mut emitter, event).unwrap();
        let event = yaml_scalar_event_new(
            None,
            None,
            "1st non-empty\n2nd non-empty 3rd non-empty",
            true,
            true,
            ScalarStyle::Plain,
        );
        yaml_emitter_emit(&mut emitter, event).unwrap();
        let event = yaml_document_end_event_new(true);
        yaml_emitter_emit(&mut emitter, event).unwrap();
        let event = yaml_stream_end_event_new();
        yaml_emitter_emit(&mut emitter, event).unwrap();

        assert_eq!(
            core::str::from_utf8(&output),
            Ok("'1st non-empty\n\n  2nd non-empty 3rd non-empty'\n")
        );
    }
}
