fn main() -> Result<(), Box<dyn std::error::Error>> {
    secretspec::cli::main().map_err(|e| e.into())
}
