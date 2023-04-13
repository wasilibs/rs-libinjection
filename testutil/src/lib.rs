use rust_embed::RustEmbed;
use std::io::BufRead;

#[derive(RustEmbed)]
#[folder = "tests/"]
struct Tests;

pub struct Test {
    pub name: String,
    pub input: String,
    pub expected: String,
}

pub fn get_sqli_tests() -> impl Iterator<Item = Test> {
    Tests::iter().map(|file| {
        let content = Tests::get(file.as_ref()).unwrap();
        let mut state = "".to_string();
        let mut input = "".to_string();
        let mut expected = "".to_string();
        for line in content.data.lines() {
            let line = line.unwrap();
            if line == "--TEST--" || line == "--INPUT--" || line == "--EXPECTED--" {
                state = line;
            } else {
                match state.as_str() {
                    "--INPUT--" => {
                        input = line;
                    }
                    "--EXPECTED--" => {
                        expected.extend(line.chars());
                    }
                    _ => {}
                }
            }
        }
        Test {
            name: file.to_string(),
            input,
            expected: expected.trim().to_string(),
        }
    })
}
