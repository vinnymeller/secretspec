// These tests verify that the macro generates compilable code
// and handles errors appropriately

use secretspec_derive::define_secrets;

#[test]
fn test_file_not_found() {
    // This should produce a compile error
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/file_not_found.rs");
}

#[test]
fn test_invalid_toml() {
    // This should produce a compile error
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/invalid_toml.rs");
}

#[test]
fn test_valid_generation() {
    // This should compile successfully
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/valid_generation.rs");
}