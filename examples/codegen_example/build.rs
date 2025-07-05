fn main() {
    // Generate typed struct from secretspec.toml
    secretspec::codegen::generate_types(
        "secretspec.toml",
        &std::env::var("OUT_DIR").unwrap()
    ).unwrap();
}