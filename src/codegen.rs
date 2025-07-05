use crate::{ProjectConfig, Result, SecretSpecError};
use quote::{format_ident, quote};
use std::fs;
use std::path::Path;

pub fn generate_types<P: AsRef<Path>>(
    toml_path: P,
    out_dir: P,
) -> Result<()> {
    let toml_content = fs::read_to_string(&toml_path)
        .map_err(|_| SecretSpecError::NoManifest)?;
    let config: ProjectConfig = toml::from_str(&toml_content)?;
    
    let struct_name = format_ident!("SecretSpec");
    let mut fields = vec![];
    let mut field_assignments = vec![];
    let mut env_var_sets = vec![];
    
    for (name, secret_config) in &config.secrets {
        let field_name = format_ident!("{}", name.to_lowercase().replace('_', "_"));
        let secret_name = name.clone();
        
        if secret_config.required && secret_config.default.is_none() {
            fields.push(quote! {
                pub #field_name: String
            });
            field_assignments.push(quote! {
                #field_name: secrets.get(#secret_name)
                    .ok_or_else(|| secretspec::SecretSpecError::RequiredSecretMissing(#secret_name.to_string()))?
                    .clone()
            });
            env_var_sets.push(quote! {
                unsafe {
                    std::env::set_var(#secret_name, &self.#field_name);
                }
            });
        } else {
            fields.push(quote! {
                pub #field_name: Option<String>
            });
            field_assignments.push(quote! {
                #field_name: secrets.get(#secret_name).cloned()
            });
            env_var_sets.push(quote! {
                if let Some(ref value) = self.#field_name {
                    unsafe {
                        std::env::set_var(#secret_name, value);
                    }
                }
            });
        }
    }
    
    let generated = quote! {
        #[derive(Debug)]
        pub struct #struct_name {
            #(#fields,)*
        }
        
        impl #struct_name {
            pub fn load() -> Result<Self, secretspec::SecretSpecError> {
                let spec = secretspec::SecretSpec::load()?;
                let secrets = spec.get_all_secrets(None, None)?;
                
                Ok(Self {
                    #(#field_assignments,)*
                })
            }
            
            pub fn load_with(provider: secretspec::codegen::Provider, profile: secretspec::codegen::Profile) -> Result<Self, secretspec::SecretSpecError> {
                let spec = secretspec::SecretSpec::load()?;
                let provider_str = match provider {
                    secretspec::codegen::Provider::Keyring => "keyring",
                    secretspec::codegen::Provider::Dotenv => "dotenv",
                    secretspec::codegen::Provider::Env => "env",
                };
                let profile_str = match profile {
                    secretspec::codegen::Profile::Development => "development",
                    secretspec::codegen::Profile::Production => "production",
                    secretspec::codegen::Profile::Staging => "staging",
                    secretspec::codegen::Profile::Test => "test",
                };
                
                let secrets = spec.get_all_secrets(Some(provider_str.to_string()), Some(profile_str.to_string()))?;
                
                Ok(Self {
                    #(#field_assignments,)*
                })
            }
            
            pub fn set_as_env_vars(&self) -> Result<(), std::io::Error> {
                #(#env_var_sets)*
                Ok(())
            }
        }
    };
    
    let out_path = out_dir.as_ref().join("secrets.rs");
    fs::write(out_path, generated.to_string())?;
    
    Ok(())
}

#[derive(Debug)]
pub enum Provider {
    Keyring,
    Dotenv,
    Env,
}

#[derive(Debug)]
pub enum Profile {
    Development,
    Production,
    Staging,
    Test,
}