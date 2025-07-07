use secretspec_derive::define_secrets;

// This should compile successfully
define_secrets!("secretspec-derive/tests/fixtures/basic.toml");

fn main() {
    // Verify the generated types exist
    let _ = std::mem::size_of::<SecretSpec>();
    
    // Verify the Provider enum was generated
    let _ = Provider::Keyring;
    
    // Verify the load method exists
    fn test_load() -> Result<SecretSpec, secretspec::SecretSpecError> {
        SecretSpec::load(Provider::Keyring)
    }
}