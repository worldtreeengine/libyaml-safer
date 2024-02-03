#![no_main]

use libfuzzer_sys::fuzz_target;
use libyaml_safer::{
    yaml_document_get_root_node, yaml_parser_load, yaml_parser_new, yaml_parser_set_input,
};

fuzz_target!(|data: &[u8]| unsafe { fuzz_target(data) });

unsafe fn fuzz_target(mut data: &[u8]) {
    let mut parser = yaml_parser_new();
    yaml_parser_set_input(&mut parser, &mut data);

    while let Ok(mut document) = yaml_parser_load(&mut parser) {
        let done = yaml_document_get_root_node(&mut document).is_none();
        if done {
            break;
        }
    }
}
