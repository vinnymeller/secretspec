use proc_macro::TokenStream;
use quote::{format_ident, quote};
use secretspec_types::ProjectConfig;
use std::collections::{HashMap, HashSet};
use syn::{LitStr, parse_macro_input};

/// Generates typed SecretSpec structs from your secretspec.toml file.
///
/// # Example
/// ```ignore
/// // In your main.rs or lib.rs:
/// secretspec::define_secrets!("secretspec.toml");
///
/// use secretspec::macros::Provider;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Load with union types (safe for any profile)
///     let secrets = SecretSpec::load(Some(Provider::Keyring), None)?;
///     println!("Database URL: {}", secrets.database_url);
///
///     // Load with profile-specific types
///     match SecretSpec::load_as_profile(Some(Provider::Keyring), Some(Profile::Production))? {
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

    let config: ProjectConfig =
        match secretspec_types::parse_spec_from_str(&toml_content, Some(&full_path)) {
            Ok(config) => config,
            Err(e) => {
                let error = format!("Failed to parse TOML: {}", e);
                return quote! { compile_error!(#error); }.into();
            }
        };

    // Collect all profiles and determine field types
    let mut all_profiles = HashSet::new();
    let mut field_info = HashMap::new();

    for (profile_name, profile_config) in &config.profiles {
        all_profiles.insert(profile_name.clone());

        for (secret_name, secret_config) in &profile_config.secrets {
            let mut is_ever_optional = false;

            // Check requirement across all profiles
            if !secret_config.required || secret_config.default.is_some() {
                is_ever_optional = true;
            }

            // Check if this secret exists in other profiles with different requirements
            for (other_profile_name, other_profile_config) in &config.profiles {
                if other_profile_name != profile_name {
                    if let Some(other_secret_config) = other_profile_config.secrets.get(secret_name)
                    {
                        let has_default = other_secret_config.default.is_some();
                        if !other_secret_config.required || has_default {
                            is_ever_optional = true;
                        }
                    } else {
                        // If secret doesn't exist in other profile, it's optional
                        is_ever_optional = true;
                    }
                }
            }

            // If it's ever optional, make it Option<String>
            let field_type = if is_ever_optional {
                quote! { Option<String> }
            } else {
                quote! { String }
            };

            field_info.insert(secret_name.clone(), field_type);
        }
    }

    // Generate the main struct
    let struct_fields = field_info.iter().map(|(name, field_type)| {
        let field_name = format_ident!("{}", name.to_lowercase());
        quote! { pub #field_name: #field_type }
    });

    // Generate field assignments for load()
    let load_assignments: Vec<_> = field_info.iter().map(|(name, field_type)| {
        let field_name = format_ident!("{}", name.to_lowercase());
        let secret_name = name.clone();
        // Check if this is an Option type by looking at the field_type
        let is_optional = field_type.to_string().starts_with("Option");

        if is_optional {
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
    }).collect();

    // Generate env var setters
    let env_setters = field_info.iter().map(|(name, field_type)| {
        let field_name = format_ident!("{}", name.to_lowercase());
        let env_name = name.clone();

        // Check if this is an Option type by looking at the field_type
        let is_optional = field_type.to_string().starts_with("Option");

        if is_optional {
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
        all_profiles
            .iter()
            .map(|name| {
                let variant = format_ident!("{}", capitalize_first(name));
                quote! { #variant }
            })
            .collect()
    };

    let profile_to_str = if all_profiles.is_empty() {
        vec![quote! { Profile::Default => "default" }]
    } else {
        all_profiles
            .iter()
            .map(|name| {
                let variant = format_ident!("{}", capitalize_first(name));
                let str_val = name.clone();
                quote! { Profile::#variant => #str_val }
            })
            .collect()
    };

    // Generate string to Profile enum mapping for TryFrom
    let str_to_profile_try = if all_profiles.is_empty() {
        vec![quote! { "default" => Ok(Profile::Default) }]
    } else {
        all_profiles
            .iter()
            .map(|name| {
                let variant = format_ident!("{}", capitalize_first(name));
                let str_val = name.clone();
                quote! { #str_val => Ok(Profile::#variant) }
            })
            .collect()
    };

    // Get first profile variant for defaults
    let first_profile_variant = if all_profiles.is_empty() {
        format_ident!("Default")
    } else {
        let first_profile = all_profiles
            .iter()
            .next()
            .expect("all_profiles should have at least one element when not empty");
        format_ident!("{}", capitalize_first(first_profile))
    };

    // Generate SecretSpecProfile enum variants
    let profile_enum_variants: Vec<_> = if all_profiles.is_empty() {
        // If no profiles, create a Default variant with all fields
        let fields = field_info.iter().map(|(secret_name, field_type)| {
            let field_name = format_ident!("{}", secret_name.to_lowercase());
            quote! { #field_name: #field_type }
        });
        vec![quote! {
            Default {
                #(#fields,)*
            }
        }]
    } else {
        all_profiles
            .iter()
            .map(|profile| {
                let variant_name = format_ident!("{}", capitalize_first(profile));
                let fields = field_info
                    .iter()
                    .filter_map(|(secret_name, _union_field_type)| {
                        let field_name = format_ident!("{}", secret_name.to_lowercase());

                        // Determine field type for this specific profile
                        let profile_field_type = if let Some(profile_config) =
                            config.profiles.get(profile)
                        {
                            if let Some(secret_config) = profile_config.secrets.get(secret_name) {
                                // Check if it's optional in this specific profile
                                if !secret_config.required || secret_config.default.is_some() {
                                    quote! { Option<String> }
                                } else {
                                    quote! { String }
                                }
                            } else {
                                // Secret doesn't exist in this profile, skip it
                                return None;
                            }
                        } else {
                            // Profile doesn't exist, skip
                            return None;
                        };

                        Some(quote! { #field_name: #profile_field_type })
                    });

                quote! {
                    #variant_name {
                        #(#fields,)*
                    }
                }
            })
            .collect()
    };

    // Generate load_profile match arms
    let load_profile_arms: Vec<_> = if all_profiles.is_empty() {
        // If no profiles, handle Default
        let assignments = field_info.iter().map(|(secret_name, field_type)| {
            let field_name = format_ident!("{}", secret_name.to_lowercase());

            // Check if this is an Option type by looking at the field_type
            let is_optional = field_type.to_string().starts_with("Option");
            if is_optional {
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

        vec![quote! {
            Profile::Default => Ok(SecretSpecProfile::Default {
                #(#assignments,)*
            })
        }]
    } else {
        all_profiles.iter().map(|profile| {
        let variant_name = format_ident!("{}", capitalize_first(profile));
        let assignments = field_info.iter().filter_map(|(secret_name, _field_type)| {
            let field_name = format_ident!("{}", secret_name.to_lowercase());

            // Check if this secret exists in this profile
            if let Some(profile_config) = config.profiles.get(profile) {
                if let Some(secret_config) = profile_config.secrets.get(secret_name) {
                    // Check if this is an Option type
                    let is_optional = !secret_config.required || secret_config.default.is_some();

                    if is_optional {
                        Some(quote! {
                            #field_name: secrets.get(#secret_name).cloned()
                        })
                    } else {
                        Some(quote! {
                            #field_name: secrets.get(#secret_name)
                                .ok_or_else(|| secretspec::SecretSpecError::RequiredSecretMissing(#secret_name.to_string()))?
                                .clone()
                        })
                    }
                } else {
                    // Secret doesn't exist in this profile, skip it
                    None
                }
            } else {
                // Profile doesn't exist, skip
                None
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

        impl std::convert::TryFrom<&str> for Profile {
            type Error = secretspec::SecretSpecError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                match value {
                    #(#str_to_profile_try,)*
                    _ => Err(secretspec::SecretSpecError::InvalidProfile(value.to_string())),
                }
            }
        }

        impl std::convert::TryFrom<String> for Profile {
            type Error = secretspec::SecretSpecError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Profile::try_from(value.as_str())
            }
        }

        // Use Provider from secretspec_types
        pub use secretspec::Provider;

        // Type alias to help with type inference
        type LoadResult<T> = Result<T, secretspec::SecretSpecError>;

        pub struct SecretSpecBuilder {
            provider: Option<Box<dyn FnOnce() -> Result<http::Uri, String>>>,
            profile: Option<Box<dyn FnOnce() -> Result<Profile, String>>>,
        }

        impl Default for SecretSpecBuilder {
            fn default() -> Self {
                Self::new()
            }
        }

        impl SecretSpecBuilder {
            pub fn new() -> Self {
                Self {
                    provider: None,
                    profile: None,
                }
            }

            pub fn with_provider<T>(mut self, provider: T) -> Self
            where
                T: TryInto<http::Uri> + 'static,
                T::Error: std::fmt::Display + 'static
            {
                self.provider = Some(Box::new(move || {
                    provider.try_into()
                        .map_err(|e| format!("Invalid provider URI: {}", e))
                }));
                self
            }

            pub fn with_profile<T>(mut self, profile: T) -> Self
            where
                T: TryInto<Profile> + 'static,
                T::Error: std::fmt::Display + 'static
            {
                self.profile = Some(Box::new(move || {
                    profile.try_into()
                        .map_err(|e| format!("{}", e))
                }));
                self
            }

            pub fn load(self) -> Result<secretspec::SecretSpecSecrets<SecretSpec>, secretspec::SecretSpecError> {
                let spec = secretspec::SecretSpec::load()?;

                // Resolve provider conversion
                let provider_str = if let Some(provider_fn) = self.provider {
                    let uri = provider_fn()
                        .map_err(|e| secretspec::SecretSpecError::ProviderOperationFailed(e))?;
                    Some(uri.to_string())
                } else {
                    None
                };

                // Resolve profile conversion
                let profile_str = if let Some(profile_fn) = self.profile {
                    let profile = profile_fn()
                        .map_err(|e| secretspec::SecretSpecError::InvalidProfile(e))?;
                    Some(match profile {
                        #(#profile_to_str,)*
                    }.to_string())
                } else {
                    None
                };

                let validation_result = spec.validate(provider_str, profile_str)?;
                let secrets = validation_result.secrets;

                let data = SecretSpec {
                    #(#load_assignments,)*
                };

                Ok(secretspec::SecretSpecSecrets::new(
                    data,
                    validation_result.provider,
                    validation_result.profile,
                ))
            }

            pub fn load_profile(self) -> Result<secretspec::SecretSpecSecrets<SecretSpecProfile>, secretspec::SecretSpecError> {
                let spec = secretspec::SecretSpec::load()?;

                // Resolve provider conversion
                let provider_str = if let Some(provider_fn) = self.provider {
                    let uri = provider_fn()
                        .map_err(|e| secretspec::SecretSpecError::ProviderOperationFailed(e))?;
                    Some(uri.to_string())
                } else {
                    None
                };

                // Resolve profile conversion
                let (profile_str, selected_profile) = if let Some(profile_fn) = self.profile {
                    let profile = profile_fn()
                        .map_err(|e| secretspec::SecretSpecError::InvalidProfile(e))?;
                    let profile_str = match profile {
                        #(#profile_to_str,)*
                    }.to_string();
                    (Some(profile_str), profile)
                } else {
                    // Check env var for profile
                    let profile_str = std::env::var("SECRETSPEC_PROFILE").ok();
                    let selected_profile = if let Some(ref profile_name) = profile_str {
                        Profile::try_from(profile_name.as_str())?
                    } else {
                        Profile::#first_profile_variant
                    };
                    (profile_str, selected_profile)
                };

                let validation_result = spec.validate(provider_str, profile_str)?;
                let secrets = validation_result.secrets;

                let data_result: LoadResult<SecretSpecProfile> = match selected_profile {
                    #(#load_profile_arms,)*
                };
                let data = data_result?;

                Ok(secretspec::SecretSpecSecrets::new(
                    data,
                    validation_result.provider,
                    validation_result.profile,
                ))
            }
        }

        impl SecretSpec {
            /// Create a new builder for loading secrets
            pub fn builder() -> SecretSpecBuilder {
                SecretSpecBuilder::new()
            }

            /// Load secrets with optional provider and/or profile
            /// If provider is None, uses SECRETSPEC_PROVIDER env var or global config
            /// If profile is None, uses SECRETSPEC_PROFILE env var if set
            pub fn load(provider: Option<Provider>, profile: Option<Profile>) -> Result<secretspec::SecretSpecSecrets<Self>, secretspec::SecretSpecError> {
                let spec = secretspec::SecretSpec::load()?;

                // Convert provider enum to string if provided, otherwise check env var
                let provider_str = match provider {
                    Some(p) => Some(p.to_string()),
                    None => std::env::var("SECRETSPEC_PROVIDER").ok(),
                };

                // Convert profile enum to string if provided, otherwise check env var
                let profile_str = match profile {
                    Some(p) => Some(match p {
                        #(#profile_to_str,)*
                    }.to_string()),
                    None => std::env::var("SECRETSPEC_PROFILE").ok(),
                };

                let validation_result = spec.validate(provider_str, profile_str)?;
                let secrets = validation_result.secrets;

                let data = Self {
                    #(#load_assignments,)*
                };

                Ok(secretspec::SecretSpecSecrets::new(
                    data,
                    validation_result.provider,
                    validation_result.profile,
                ))
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
