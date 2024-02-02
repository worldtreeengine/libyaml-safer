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
    yaml_parser_delete, yaml_parser_new, yaml_parser_parse, yaml_parser_set_input, YamlEventData,
    YAML_DOUBLE_QUOTED_SCALAR_STYLE, YAML_FOLDED_SCALAR_STYLE, YAML_LITERAL_SCALAR_STYLE,
    YAML_PLAIN_SCALAR_STYLE, YAML_SINGLE_QUOTED_SCALAR_STYLE,
};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::process::{self, ExitCode};
use std::slice;

pub(crate) fn test_main(
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    let mut parser = yaml_parser_new();

    yaml_parser_set_input(&mut parser, stdin);

    loop {
        let event = match yaml_parser_parse(&mut parser) {
            Err(err) => {
                let error = format!("Parse error: {}", err);
                yaml_parser_delete(&mut parser);
                return Err(error.into());
            }
            Ok(event) => event,
        };

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

        if is_end {
            break;
        }
    }
    yaml_parser_delete(&mut parser);
    Ok(())
}

fn print_escaped(stdout: &mut dyn Write, s: &str) {
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
        let result = test_main(&mut stdin, &mut stdout);
        if let Err(err) = result {
            let _ = writeln!(io::stderr(), "{}", err);
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
