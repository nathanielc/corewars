use std::fs;
use std::path::Path;

use corewars::{
    core::{Core, CoreConfig},
    parser,
};

#[test]
fn validate_redcode() {
    // Workaround for the fact that `test_resources` paths are based on workspace Cargo.toml
    // https://github.com/frehberg/test-generator/issues/6
    let input_file =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/input/simple/validate.redcode");

    let input = fs::read_to_string(input_file).unwrap();
    let warrior = parser::parse(&input).unwrap();

    // hmm, it would be useful to keep the labelmap around for analysis here...

    let mut core = Core::new(CoreConfig {
        core_size: 8_000,
        max_cycles: 10_000,
        ..CoreConfig::default()
    });
    core.load_warriors(&vec![warrior]).unwrap();

    eprintln!("Before run:\n{:?}\n==============================", core);

    core.run();
}
