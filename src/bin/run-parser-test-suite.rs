#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::let_underscore_untyped,
    clippy::missing_errors_doc,
    clippy::missing_safety_doc,
    clippy::too_many_lines
)]

use libyaml_safer::{
    yaml_event_delete, yaml_event_t, yaml_parser_delete, yaml_parser_initialize, yaml_parser_parse,
    yaml_parser_set_input, yaml_parser_t, YamlEventData, YAML_DOUBLE_QUOTED_SCALAR_STYLE,
    YAML_FOLDED_SCALAR_STYLE, YAML_LITERAL_SCALAR_STYLE, YAML_PLAIN_SCALAR_STYLE,
    YAML_SINGLE_QUOTED_SCALAR_STYLE,
};
use std::env;
use std::error::Error;
use std::ffi::c_void;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read, Write};
use std::mem::MaybeUninit;
use std::process::{self, ExitCode};
use std::ptr::addr_of_mut;
use std::slice;

pub(crate) unsafe fn unsafe_main(
    mut stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    let mut parser = MaybeUninit::<yaml_parser_t>::uninit();
    if yaml_parser_initialize(parser.as_mut_ptr()).is_err() {
        return Err("Could not initialize the parser object".into());
    }
    let mut parser = parser.assume_init();

    unsafe fn read_from_stdio(
        data: *mut c_void,
        buffer: *mut u8,
        size: u64,
        size_read: *mut u64,
    ) -> i32 {
        let stdin: *mut &mut dyn Read = data.cast();
        let slice = slice::from_raw_parts_mut(buffer.cast(), size as usize);
        match (*stdin).read(slice) {
            Ok(n) => {
                *size_read = n as u64;
                1
            }
            Err(_) => 0,
        }
    }

    yaml_parser_set_input(&mut parser, read_from_stdio, addr_of_mut!(stdin).cast());

    let mut event = yaml_event_t::default();
    loop {
        if yaml_parser_parse(&mut parser, &mut event).is_err() {
            let mut error = format!("Parse error: {}", parser.problem.unwrap_or(""));
            if parser.problem_mark.line != 0 || parser.problem_mark.column != 0 {
                let _ = write!(
                    error,
                    "\nLine: {} Column: {}",
                    (parser.problem_mark.line).wrapping_add(1_u64),
                    (parser.problem_mark.column).wrapping_add(1_u64),
                );
            }
            yaml_parser_delete(&mut parser);
            return Err(error.into());
        }

        let mut is_end = false;

        match &event.data {
            YamlEventData::NoEvent => {
                _ = writeln!(stdout, "???");
            }
            YamlEventData::StreamStart { .. } => {
                _ = writeln!(stdout, "+STR");
            }
            YamlEventData::StreamEnd => {
                is_end = true;
                _ = writeln!(stdout, "-STR");
            }
            YamlEventData::DocumentStart { implicit, .. } => {
                _ = write!(stdout, "+DOC");
                if !*implicit {
                    _ = write!(stdout, " ---");
                }
                _ = writeln!(stdout);
            }
            YamlEventData::DocumentEnd { implicit } => {
                _ = write!(stdout, "-DOC");
                if !*implicit {
                    _ = write!(stdout, " ...");
                }
                _ = writeln!(stdout);
            }
            YamlEventData::Alias { anchor } => {
                _ = writeln!(stdout, "=ALI *{}", anchor);
            }
            YamlEventData::Scalar {
                anchor,
                tag,
                value,
                style,
                ..
            } => {
                let _ = write!(stdout, "=VAL");
                if let Some(anchor) = anchor {
                    _ = write!(stdout, " &{}", anchor);
                }
                if let Some(tag) = tag {
                    _ = write!(stdout, " <{}>", tag);
                }
                _ = stdout.write_all(match *style {
                    YAML_PLAIN_SCALAR_STYLE => b" :",
                    YAML_SINGLE_QUOTED_SCALAR_STYLE => b" '",
                    YAML_DOUBLE_QUOTED_SCALAR_STYLE => b" \"",
                    YAML_LITERAL_SCALAR_STYLE => b" |",
                    YAML_FOLDED_SCALAR_STYLE => b" >",
                    _ => process::abort(),
                });
                print_escaped(stdout, &value);
                _ = writeln!(stdout);
            }
            YamlEventData::SequenceStart { anchor, tag, .. } => {
                let _ = write!(stdout, "+SEQ");
                if let Some(anchor) = anchor {
                    _ = write!(stdout, " &{}", anchor);
                }
                if let Some(tag) = tag {
                    _ = write!(stdout, " <{}>", tag);
                }
                _ = writeln!(stdout);
            }
            YamlEventData::SequenceEnd => {
                _ = writeln!(stdout, "-SEQ");
            }
            YamlEventData::MappingStart { anchor, tag, .. } => {
                let _ = write!(stdout, "+MAP");
                if let Some(anchor) = anchor {
                    _ = write!(stdout, " &{}", anchor);
                }
                if let Some(tag) = tag {
                    _ = write!(stdout, " <{}>", tag);
                }
                _ = writeln!(stdout);
            }
            YamlEventData::MappingEnd => {
                _ = writeln!(stdout, "-MAP");
            }
        }

        yaml_event_delete(&mut event);
        if is_end {
            break;
        }
    }
    yaml_parser_delete(&mut parser);
    Ok(())
}

unsafe fn print_escaped(stdout: &mut dyn Write, s: &str) {
    for ch in s.bytes() {
        let repr = match &ch {
            b'\\' => b"\\\\",
            b'\0' => b"\\0",
            b'\x08' => b"\\b",
            b'\n' => b"\\n",
            b'\r' => b"\\r",
            b'\t' => b"\\t",
            c => slice::from_ref(c),
        };
        let _ = stdout.write_all(repr);
    }
}

fn main() -> ExitCode {
    let args = env::args_os().skip(1);
    if args.len() == 0 {
        let _ = writeln!(io::stderr(), "Usage: run-parser-test-suite <in.yaml>...");
        return ExitCode::FAILURE;
    }
    for arg in args {
        let mut stdin = File::open(arg).unwrap();
        let mut stdout = io::stdout();
        let result = unsafe { unsafe_main(&mut stdin, &mut stdout) };
        if let Err(err) = result {
            let _ = writeln!(io::stderr(), "{}", err);
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
