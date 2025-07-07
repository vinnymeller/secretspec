use secretspec_derive::define_secrets;
use secretspec::codegen::Provider;

// This should compile successfully
define_secrets!("tests/fixtures/basic.toml");

fn main() {
    // Verify the generated types exist
    let _ = std::mem::size_of::<SecretSpec>();
    
    // Verify the load method exists
    fn test_load() -> Result<SecretSpec, secretspec::SecretSpecError> {
        SecretSpec::load(Provider::Keyring)
    }
}