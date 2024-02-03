#![no_main]

use libfuzzer_sys::fuzz_target;
use libyaml_safer::{yaml_parser_new, yaml_parser_parse, yaml_parser_set_input, EventData};

fuzz_target!(|data: &[u8]| unsafe { fuzz_target(data) });

unsafe fn fuzz_target(mut data: &[u8]) {
    let mut parser = yaml_parser_new();
    yaml_parser_set_input(&mut parser, &mut data);

    while let Ok(event) = yaml_parser_parse(&mut parser) {
        let is_end = matches!(event.data, EventData::StreamEnd);
        if is_end {
            break;
        }
    }
}
