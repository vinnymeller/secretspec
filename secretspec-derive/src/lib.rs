use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, LitStr};
use std::collections::{HashMap, HashSet};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    secrets: HashMap<String, SecretConfig>,
}

#[derive(Debug, Deserialize)]
struct SecretConfig {
    #[serde(default)]
    #[allow(dead_code)]
    description: Option<String>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    default: Option<String>,
    #[serde(flatten)]
    profiles: HashMap<String, ProfileOverride>,
}

#[derive(Debug, Deserialize)]
struct ProfileOverride {
    #[serde(default)]
    required: Option<bool>,
    #[serde(default)]
    default: Option<String>,
}

/// Generates typed SecretSpec structs from your secretspec.toml file.
/// 
/// # Example
/// ```ignore
/// // In your main.rs or lib.rs:
/// secretspec::define_secrets!("secretspec.toml");
/// 
/// use secretspec::codegen::Provider;
/// 
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Load with union types (safe for any profile)
///     let secrets = SecretSpec::load(Provider::Keyring)?;
///     println!("Database URL: {}", secrets.database_url);
///     
///     // Load with profile-specific types
///     match SecretSpec::load_as(Provider::Keyring, Profile::Production)? {
///         SecretSpecProfile::Production { api_key, database_url, .. } => {
///             println!("Production API key: {}", api_key);
///         }
///         _ => unreachable!(),
///     }
///     
///     Ok(())
/// }
/// ```
#[proc_macro]
pub fn define_secrets(input: TokenStream) -> TokenStream {
    let path = parse_macro_input!(input as LitStr).value();
    
    // Get the manifest directory of the crate using the macro
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let full_path = std::path::Path::new(&manifest_dir).join(&path);
    
    // Read and parse TOML at compile time
    let toml_content = match std::fs::read_to_string(&full_path) {
        Ok(content) => content,
        Err(e) => {
            let error = format!("Failed to read {}: {}", path, e);
            return quote! { compile_error!(#error); }.into();
        }
    };
    
    let config: Config = match toml::from_str(&toml_content) {
        Ok(config) => config,
        Err(e) => {
            let error = format!("Failed to parse TOML: {}", e);
            return quote! { compile_error!(#error); }.into();
        }
    };
    
    // Collect all profiles and determine field types
    let mut all_profiles = HashSet::new();
    let mut field_info = HashMap::new();
    
    for (secret_name, secret_config) in &config.secrets {
        let mut is_ever_optional = false;
        
        // Check base requirement
        if !secret_config.required || secret_config.default.is_some() {
            is_ever_optional = true;
        }
        
        // Check profile overrides
        for (profile_name, profile_override) in &secret_config.profiles {
            all_profiles.insert(profile_name.clone());
            
            let profile_required = profile_override.required.unwrap_or(secret_config.required);
            let has_default = profile_override.default.is_some() || secret_config.default.is_some();
            
            if !profile_required || has_default {
                is_ever_optional = true;
            }
        }
        
        // If it's ever optional, make it Option<String>
        let field_type = if is_ever_optional {
            quote! { Option<String> }
        } else {
            quote! { String }
        };
        
        field_info.insert(secret_name.clone(), (field_type, is_ever_optional));
    }
    
    // Generate the main struct
    let struct_fields = field_info.iter().map(|(name, (field_type, _))| {
        let field_name = format_ident!("{}", name.to_lowercase());
        quote! { pub #field_name: #field_type }
    });
    
    // Generate field assignments for load()
    let load_assignments = field_info.iter().map(|(name, (_, is_optional))| {
        let field_name = format_ident!("{}", name.to_lowercase());
        let secret_name = name.clone();
        
        if *is_optional {
            quote! {
                #field_name: secrets.get(#secret_name).cloned()
            }
        } else {
            quote! {
                #field_name: secrets.get(#secret_name)
                    .ok_or_else(|| secretspec::SecretSpecError::RequiredSecretMissing(#secret_name.to_string()))?
                    .clone()
            }
        }
    });
    
    // Generate env var setters
    let env_setters = field_info.iter().map(|(name, (_, is_optional))| {
        let field_name = format_ident!("{}", name.to_lowercase());
        let env_name = name.clone();
        
        if *is_optional {
            quote! {
                if let Some(ref value) = self.#field_name {
                    unsafe {
                        std::env::set_var(#env_name, value);
                    }
                }
            }
        } else {
            quote! {
                unsafe {
                    std::env::set_var(#env_name, &self.#field_name);
                }
            }
        }
    });
    
    // Generate Profile enum (always include Default if no profiles defined)
    let profile_variants = if all_profiles.is_empty() {
        vec![quote! { Default }]
    } else {
        all_profiles.iter().map(|name| {
            let variant = format_ident!("{}", capitalize_first(name));
            quote! { #variant }
        }).collect()
    };
    
    let profile_to_str = if all_profiles.is_empty() {
        vec![quote! { Profile::Default => "default" }]
    } else {
        all_profiles.iter().map(|name| {
            let variant = format_ident!("{}", capitalize_first(name));
            let str_val = name.clone();
            quote! { Profile::#variant => #str_val }
        }).collect()
    };
    
    // Generate SecretSpecProfile enum variants
    let profile_enum_variants: Vec<_> = if all_profiles.is_empty() {
        // If no profiles, create a Default variant with all fields
        let fields = config.secrets.iter().map(|(secret_name, secret_config)| {
            let field_name = format_ident!("{}", secret_name.to_lowercase());
            let field_type = if secret_config.required && secret_config.default.is_none() {
                quote! { String }
            } else {
                quote! { Option<String> }
            };
            quote! { #field_name: #field_type }
        });
        vec![quote! {
            Default {
                #(#fields,)*
            }
        }]
    } else {
        all_profiles.iter().map(|profile| {
        let variant_name = format_ident!("{}", capitalize_first(profile));
        let fields = config.secrets.iter().map(|(secret_name, secret_config)| {
            let field_name = format_ident!("{}", secret_name.to_lowercase());
            
            // Determine if this field is required for this profile
            let mut is_required = secret_config.required;
            let mut has_default = secret_config.default.is_some();
            
            if let Some(profile_override) = secret_config.profiles.get(profile) {
                is_required = profile_override.required.unwrap_or(is_required);
                has_default = profile_override.default.is_some() || has_default;
            }
            
            let field_type = if is_required && !has_default {
                quote! { String }
            } else {
                quote! { Option<String> }
            };
            
            quote! { #field_name: #field_type }
        });
        
        quote! {
            #variant_name {
                #(#fields,)*
            }
        }
    }).collect()
    };
    
    // Generate load_as match arms
    let load_as_arms: Vec<_> = if all_profiles.is_empty() {
        // If no profiles, handle Default
        let assignments = config.secrets.iter().map(|(secret_name, secret_config)| {
            let field_name = format_ident!("{}", secret_name.to_lowercase());
            
            if secret_config.required && secret_config.default.is_none() {
                quote! {
                    #field_name: secrets.get(#secret_name)
                        .ok_or_else(|| secretspec::SecretSpecError::RequiredSecretMissing(#secret_name.to_string()))?
                        .clone()
                }
            } else {
                quote! {
                    #field_name: secrets.get(#secret_name).cloned()
                }
            }
        });
        
        vec![quote! {
            Profile::Default => Ok(SecretSpecProfile::Default {
                #(#assignments,)*
            })
        }]
    } else {
        all_profiles.iter().map(|profile| {
        let variant_name = format_ident!("{}", capitalize_first(profile));
        let assignments = config.secrets.iter().map(|(secret_name, secret_config)| {
            let field_name = format_ident!("{}", secret_name.to_lowercase());
            
            // Determine if this field is required for this profile
            let mut is_required = secret_config.required;
            let mut has_default = secret_config.default.is_some();
            
            if let Some(profile_override) = secret_config.profiles.get(profile) {
                is_required = profile_override.required.unwrap_or(is_required);
                has_default = profile_override.default.is_some() || has_default;
            }
            
            if is_required && !has_default {
                quote! {
                    #field_name: secrets.get(#secret_name)
                        .ok_or_else(|| secretspec::SecretSpecError::RequiredSecretMissing(#secret_name.to_string()))?
                        .clone()
                }
            } else {
                quote! {
                    #field_name: secrets.get(#secret_name).cloned()
                }
            }
        });
        
        quote! {
            Profile::#variant_name => Ok(SecretSpecProfile::#variant_name {
                #(#assignments,)*
            })
        }
    }).collect()
    };
    
    let output = quote! {
        #[derive(Debug)]
        pub struct SecretSpec {
            #(#struct_fields,)*
        }
        
        #[derive(Debug)]
        pub enum SecretSpecProfile {
            #(#profile_enum_variants,)*
        }
        
        #[derive(Debug, Clone, Copy)]
        pub enum Profile {
            #(#profile_variants,)*
        }
        
        /// Provider enum for typed access to secret providers
        #[derive(Debug, Clone, Copy)]
        pub enum Provider {
            Keyring,
            Dotenv,
            Env,
        }
        
        impl SecretSpec {
            /// Load with provider, returns union type (safest for all profiles)
            pub fn load(provider: Provider) -> Result<Self, secretspec::SecretSpecError> {
                let spec = secretspec::SecretSpec::load()?;
                let provider_str = match provider {
                    Provider::Keyring => "keyring",
                    Provider::Dotenv => "dotenv",
                    Provider::Env => "env",
                };
                
                let secrets = spec.get_all_secrets(Some(provider_str.to_string()), None)?;
                
                Ok(Self {
                    #(#load_assignments,)*
                })
            }
            
            /// Load with specific provider and profile, returns profile-specific types
            pub fn load_as(provider: Provider, profile: Profile) -> Result<SecretSpecProfile, secretspec::SecretSpecError> {
                let spec = secretspec::SecretSpec::load()?;
                let provider_str = match provider {
                    Provider::Keyring => "keyring",
                    Provider::Dotenv => "dotenv",
                    Provider::Env => "env",
                };
                let profile_str = match profile {
                    #(#profile_to_str,)*
                };
                
                let secrets = spec.get_all_secrets(
                    Some(provider_str.to_string()), 
                    Some(profile_str.to_string())
                )?;
                
                match profile {
                    #(#load_as_arms,)*
                }
            }
            
            pub fn set_as_env_vars(&self) {
                #(#env_setters)*
            }
        }
    };
    
    output.into()
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;