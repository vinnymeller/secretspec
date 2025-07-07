use secretspec_derive::define_secrets;

// This should fail because the TOML is invalid
define_secrets!("tests/fixtures/invalid_toml.txt");

fn main() {}