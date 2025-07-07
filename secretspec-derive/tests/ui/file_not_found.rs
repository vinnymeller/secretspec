use secretspec_derive::define_secrets;

// This should fail because the file doesn't exist
define_secrets!("this/file/does/not/exist.toml");

fn main() {}