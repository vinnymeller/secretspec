use proc_macro::TokenStream;
use quote::{format_ident, quote};
use secretspec_types::{ProjectConfig, SecretConfig};
use std::collections::{BTreeMap, HashSet};
use syn::{LitStr, parse_macro_input};

/// Holds metadata about a field in the generated struct
#[derive(Clone)]
struct FieldInfo {
    name: String,
    field_type: proc_macro2::TokenStream,
    is_optional: bool,
}

impl FieldInfo {
    fn new(name: String, field_type: proc_macro2::TokenStream, is_optional: bool) -> Self {
        Self {
            name,
            field_type,
            is_optional,
        }
    }

    /// Get the field name as an identifier
    fn field_name(&self) -> proc_macro2::Ident {
        field_name_ident(&self.name)
    }

    /// Generate the struct field declaration
    fn generate_struct_field(&self) -> proc_macro2::TokenStream {
        let field_name = self.field_name();
        let field_type = &self.field_type;
        quote! { pub #field_name: #field_type }
    }

    /// Generate a field assignment from a secrets map
    fn generate_assignment(&self, source: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        generate_secret_assignment(&self.field_name(), &self.name, source, self.is_optional)
    }

    /// Generate environment variable setter
    fn generate_env_setter(&self) -> proc_macro2::TokenStream {
        let field_name = self.field_name();
        let env_name = &self.name;

        if self.is_optional {
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
    }
}

/// Profile variant information
struct ProfileVariant {
    name: String,
    capitalized: String,
}

impl ProfileVariant {
    fn new(name: String) -> Self {
        let capitalized = capitalize_first(&name);
        Self { name, capitalized }
    }

    fn as_ident(&self) -> proc_macro2::Ident {
        format_ident!("{}", self.capitalized)
    }
}

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

    // Validate the configuration at compile time
    if let Err(validation_errors) = validate_config_for_codegen(&config) {
        let error_message = format!(
            "Invalid secretspec configuration:\n{}",
            validation_errors.join("\n")
        );
        return quote! { compile_error!(#error_message); }.into();
    }

    // Generate all the code
    let output = generate_secret_spec_code(config);
    output.into()
}

// ===== Core Helper Functions =====

/// Validate configuration for code generation concerns only
/// This is different from runtime validation - we only check things that would
/// prevent generating valid Rust code
fn validate_config_for_codegen(config: &ProjectConfig) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    // Validate secret names produce valid Rust identifiers
    validate_rust_identifiers(config, &mut errors);

    // Validate profile names produce valid Rust enum variants
    validate_profile_identifiers(config, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate all secret names produce valid Rust identifiers
fn validate_rust_identifiers(config: &ProjectConfig, errors: &mut Vec<String>) {
    let rust_keywords = [
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do",
        "final", "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
    ];

    for (profile_name, profile_config) in &config.profiles {
        let mut profile_field_names = HashSet::new();

        for secret_name in profile_config.secrets.keys() {
            let field_name = secret_name.to_lowercase();

            // Check if it produces a valid Rust identifier
            if !is_valid_rust_identifier(&field_name) {
                errors.push(format!(
                    "Secret '{}' in profile '{}' produces invalid Rust field name '{}'",
                    secret_name, profile_name, field_name
                ));
            }

            // Check for Rust keywords
            if rust_keywords.contains(&field_name.as_str()) {
                errors.push(format!(
                    "Secret '{}' in profile '{}' produces Rust keyword '{}' as field name",
                    secret_name, profile_name, field_name
                ));
            }

            // Check for duplicate field names within the same profile
            if !profile_field_names.insert(field_name.clone()) {
                errors.push(format!(
                    "Profile '{}' has multiple secrets that produce the same field name '{}' (names are case-insensitive)",
                    profile_name, field_name
                ));
            }
        }
    }
}

/// Check if a string is a valid Rust identifier
fn is_valid_rust_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let mut chars = s.chars();
    if let Some(first) = chars.next() {
        // First character must be alphabetic or underscore
        if !first.is_alphabetic() && first != '_' {
            return false;
        }
        // Remaining characters must be alphanumeric or underscore
        chars.all(|c| c.is_alphanumeric() || c == '_')
    } else {
        false
    }
}

/// Validate profile names produce valid Rust enum variants
fn validate_profile_identifiers(config: &ProjectConfig, errors: &mut Vec<String>) {
    for profile_name in config.profiles.keys() {
        let variant_name = capitalize_first(profile_name);
        if !is_valid_rust_identifier(&variant_name) {
            errors.push(format!(
                "Profile '{}' produces invalid Rust enum variant '{}'",
                profile_name, variant_name
            ));
        }
    }
}

/// Convert a secret name to a field identifier
fn field_name_ident(name: &str) -> proc_macro2::Ident {
    format_ident!("{}", name.to_lowercase())
}

/// Helper function to check if a secret is optional
fn is_secret_optional(secret_config: &SecretConfig) -> bool {
    !secret_config.required || secret_config.default.is_some()
}

/// Determines if a field should be optional across all profiles
fn is_field_optional_across_profiles(secret_name: &str, config: &ProjectConfig) -> bool {
    // Check each profile
    for (_, profile_config) in &config.profiles {
        if let Some(secret_config) = profile_config.secrets.get(secret_name) {
            if is_secret_optional(secret_config) {
                return true;
            }
        } else {
            // Secret doesn't exist in this profile, so it's optional
            return true;
        }
    }
    false
}

/// Generate a unified secret assignment
fn generate_secret_assignment(
    field_name: &proc_macro2::Ident,
    secret_name: &str,
    source: proc_macro2::TokenStream,
    is_optional: bool,
) -> proc_macro2::TokenStream {
    if is_optional {
        quote! {
            #field_name: #source.get(#secret_name).cloned()
        }
    } else {
        quote! {
            #field_name: #source.get(#secret_name)
                .ok_or_else(|| secretspec::SecretSpecError::RequiredSecretMissing(#secret_name.to_string()))?
                .clone()
        }
    }
}

/// Analyzes all profiles to determine field types for the union struct
fn analyze_field_types(config: &ProjectConfig) -> BTreeMap<String, FieldInfo> {
    let mut field_info = BTreeMap::new();

    // Collect all unique secrets across all profiles
    for (_, profile_config) in &config.profiles {
        for (secret_name, _) in &profile_config.secrets {
            field_info.entry(secret_name.clone()).or_insert_with(|| {
                let is_optional = is_field_optional_across_profiles(secret_name, config);
                let field_type = if is_optional {
                    quote! { Option<String> }
                } else {
                    quote! { String }
                };
                FieldInfo::new(secret_name.clone(), field_type, is_optional)
            });
        }
    }

    field_info
}

/// Get normalized profile variants
fn get_profile_variants(profiles: &HashSet<String>) -> Vec<ProfileVariant> {
    if profiles.is_empty() {
        vec![ProfileVariant::new("default".to_string())]
    } else {
        let mut variants: Vec<_> = profiles
            .iter()
            .map(|name| ProfileVariant::new(name.clone()))
            .collect();
        variants.sort_by(|a, b| a.name.cmp(&b.name));
        variants
    }
}

// ===== Profile Generation Module =====

mod profile_generation {
    use super::*;

    /// Generate just the Profile enum
    pub fn generate_enum(variants: &[ProfileVariant]) -> proc_macro2::TokenStream {
        let enum_variants = variants.iter().map(|v| {
            let ident = v.as_ident();
            quote! { #ident }
        });

        quote! {
            #[derive(Debug, Clone, Copy)]
            pub enum Profile {
                #(#enum_variants,)*
            }
        }
    }

    /// Generate TryFrom implementations for Profile
    pub fn generate_try_from_impls(variants: &[ProfileVariant]) -> proc_macro2::TokenStream {
        let from_str_arms = variants.iter().map(|v| {
            let ident = v.as_ident();
            let str_val = &v.name;
            quote! { #str_val => Ok(Profile::#ident) }
        });

        quote! {
            impl std::convert::TryFrom<&str> for Profile {
                type Error = secretspec::SecretSpecError;

                fn try_from(value: &str) -> Result<Self, Self::Error> {
                    match value {
                        #(#from_str_arms,)*
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
        }
    }

    /// Generate as_str implementation for Profile
    pub fn generate_as_str_impl(variants: &[ProfileVariant]) -> proc_macro2::TokenStream {
        let to_str_arms = variants.iter().map(|v| {
            let ident = v.as_ident();
            let str_val = &v.name;
            quote! { Profile::#ident => #str_val }
        });

        quote! {
            impl Profile {
                fn as_str(&self) -> &'static str {
                    match self {
                        #(#to_str_arms,)*
                    }
                }
            }
        }
    }

    /// Generate all profile-related code
    pub fn generate_all(variants: &[ProfileVariant]) -> proc_macro2::TokenStream {
        let enum_def = generate_enum(variants);
        let try_from_impls = generate_try_from_impls(variants);
        let as_str_impl = generate_as_str_impl(variants);

        quote! {
            #enum_def
            #try_from_impls
            #as_str_impl
        }
    }
}

// ===== SecretSpec Generation Module =====

mod secret_spec_generation {
    use super::*;

    /// Generate the SecretSpec struct
    pub fn generate_struct(field_info: &BTreeMap<String, FieldInfo>) -> proc_macro2::TokenStream {
        let fields = field_info.values().map(|info| info.generate_struct_field());

        quote! {
            #[derive(Debug, serde::Serialize, serde::Deserialize)]
            pub struct SecretSpec {
                #(#fields,)*
            }
        }
    }

    /// Generate the SecretSpecProfile enum
    pub fn generate_profile_enum(
        profile_variants: &[proc_macro2::TokenStream],
    ) -> proc_macro2::TokenStream {
        quote! {
            #[derive(Debug, serde::Serialize, serde::Deserialize)]
            pub enum SecretSpecProfile {
                #(#profile_variants,)*
            }
        }
    }

    /// Generate SecretSpecProfile enum variants
    pub fn generate_profile_enum_variants(
        config: &ProjectConfig,
        field_info: &BTreeMap<String, FieldInfo>,
        variants: &[ProfileVariant],
    ) -> Vec<proc_macro2::TokenStream> {
        if config.profiles.is_empty() {
            // If no profiles, create a Default variant with all fields
            let fields = field_info.values().map(|info| info.generate_struct_field());
            vec![quote! {
                Default {
                    #(#fields,)*
                }
            }]
        } else {
            variants
                .iter()
                .filter_map(|variant| {
                    config.profiles.get(&variant.name).map(|profile_config| {
                        let variant_ident = variant.as_ident();
                        let fields =
                            profile_config
                                .secrets
                                .iter()
                                .map(|(secret_name, secret_config)| {
                                    let field_name = field_name_ident(secret_name);
                                    let field_type = if is_secret_optional(secret_config) {
                                        quote! { Option<String> }
                                    } else {
                                        quote! { String }
                                    };
                                    quote! { #field_name: #field_type }
                                });

                        quote! {
                            #variant_ident {
                                #(#fields,)*
                            }
                        }
                    })
                })
                .collect()
        }
    }

    /// Generate load_profile match arms
    pub fn generate_load_profile_arms(
        config: &ProjectConfig,
        field_info: &BTreeMap<String, FieldInfo>,
        variants: &[ProfileVariant],
    ) -> Vec<proc_macro2::TokenStream> {
        if config.profiles.is_empty() {
            // Handle Default profile
            let assignments = field_info
                .values()
                .map(|info| info.generate_assignment(quote! { secrets }));

            vec![quote! {
                Profile::Default => Ok(SecretSpecProfile::Default {
                    #(#assignments,)*
                })
            }]
        } else {
            variants
                .iter()
                .filter_map(|variant| {
                    config.profiles.get(&variant.name).map(|profile_config| {
                        let variant_ident = variant.as_ident();
                        let assignments =
                            profile_config
                                .secrets
                                .iter()
                                .map(|(secret_name, secret_config)| {
                                    let field_name = field_name_ident(secret_name);
                                    generate_secret_assignment(
                                        &field_name,
                                        secret_name,
                                        quote! { secrets },
                                        is_secret_optional(secret_config),
                                    )
                                });

                        quote! {
                            Profile::#variant_ident => Ok(SecretSpecProfile::#variant_ident {
                                #(#assignments,)*
                            })
                        }
                    })
                })
                .collect()
        }
    }

    /// Generate the shared load_internal implementation
    pub fn generate_load_internal() -> proc_macro2::TokenStream {
        quote! {
            fn load_internal(
                provider_str: Option<String>,
                profile_str: Option<String>,
            ) -> Result<secretspec::ValidationResult, secretspec::SecretSpecError> {
                let spec = secretspec::SecretSpec::load()?;
                spec.validate(provider_str, profile_str)
            }
        }
    }

    /// Generate SecretSpec implementation
    pub fn generate_impl(
        load_assignments: &[proc_macro2::TokenStream],
        env_setters: Vec<proc_macro2::TokenStream>,
    ) -> proc_macro2::TokenStream {
        quote! {
            impl SecretSpec {
                /// Create a new builder for loading secrets
                pub fn builder() -> SecretSpecBuilder {
                    SecretSpecBuilder::new()
                }

                /// Load secrets with optional provider and/or profile
                /// If provider is None, uses SECRETSPEC_PROVIDER env var or global config
                /// If profile is None, uses SECRETSPEC_PROFILE env var if set
                pub fn load(provider: Option<Provider>, profile: Option<Profile>) -> Result<secretspec::SecretSpecSecrets<Self>, secretspec::SecretSpecError> {
                    // Convert options to strings
                    let provider_str = match provider {
                        Some(p) => Some(p.to_string()),
                        None => std::env::var("SECRETSPEC_PROVIDER").ok(),
                    };

                    let profile_str = match profile {
                        Some(p) => Some(p.as_str().to_string()),
                        None => std::env::var("SECRETSPEC_PROFILE").ok(),
                    };

                    let validation_result = load_internal(provider_str, profile_str)?;
                    let secrets = validation_result.secrets;

                    let data = Self {
                        #(#load_assignments,)*
                    };

                    Ok(secretspec::SecretSpecSecrets::new(
                        data,
                        validation_result.provider,
                        validation_result.profile
                    ))
                }

                pub fn set_as_env_vars(&self) {
                    #(#env_setters)*
                }
            }
        }
    }
}

// ===== Builder Generation Module =====

mod builder_generation {
    use super::*;

    /// Generate the builder struct definition
    pub fn generate_struct() -> proc_macro2::TokenStream {
        quote! {
            pub struct SecretSpecBuilder {
                provider: Option<Box<dyn FnOnce() -> Result<http::Uri, String>>>,
                profile: Option<Box<dyn FnOnce() -> Result<Profile, String>>>,
            }
        }
    }

    /// Generate builder basic methods
    pub fn generate_basic_methods() -> proc_macro2::TokenStream {
        quote! {
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
            }
        }
    }

    /// Generate provider resolution logic
    fn generate_provider_resolution(
        provider_expr: proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        quote! {
            let provider_str = if let Some(provider_fn) = #provider_expr {
                let uri = provider_fn()
                    .map_err(|e| secretspec::SecretSpecError::ProviderOperationFailed(e))?;
                Some(uri.to_string())
            } else {
                None
            };
        }
    }

    /// Generate profile resolution logic
    fn generate_profile_resolution(
        profile_expr: proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        quote! {
            let profile_str = if let Some(profile_fn) = #profile_expr {
                let profile = profile_fn()
                    .map_err(|e| secretspec::SecretSpecError::InvalidProfile(e))?;
                Some(profile.as_str().to_string())
            } else {
                None
            };
        }
    }

    /// Generate load methods
    pub fn generate_load_methods(
        load_assignments: &[proc_macro2::TokenStream],
        load_profile_arms: &[proc_macro2::TokenStream],
        first_profile_variant: &proc_macro2::Ident,
    ) -> proc_macro2::TokenStream {
        let resolve_provider_load = generate_provider_resolution(quote! { self.provider.take() });
        let resolve_profile_load = generate_profile_resolution(quote! { self.profile.take() });
        let resolve_provider_profile =
            generate_provider_resolution(quote! { self.provider.take() });

        quote! {
            impl SecretSpecBuilder {
                pub fn load(mut self) -> Result<secretspec::SecretSpecSecrets<SecretSpec>, secretspec::SecretSpecError> {
                    #resolve_provider_load
                    #resolve_profile_load

                    let validation_result = load_internal(provider_str, profile_str)?;
                    let secrets = validation_result.secrets;

                    let data = SecretSpec {
                        #(#load_assignments,)*
                    };

                    Ok(secretspec::SecretSpecSecrets::new(
                        data,
                        validation_result.provider,
                        validation_result.profile
                    ))
                }

                pub fn load_profile(mut self) -> Result<secretspec::SecretSpecSecrets<SecretSpecProfile>, secretspec::SecretSpecError> {
                    #resolve_provider_profile

                    let (profile_str, selected_profile) = if let Some(profile_fn) = self.profile.take() {
                        let profile = profile_fn()
                            .map_err(|e| secretspec::SecretSpecError::InvalidProfile(e))?;
                        (Some(profile.as_str().to_string()), profile)
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

                    let validation_result = load_internal(provider_str, profile_str)?;
                    let secrets = validation_result.secrets;

                    let data_result: LoadResult<SecretSpecProfile> = match selected_profile {
                        #(#load_profile_arms,)*
                    };
                    let data = data_result?;

                    Ok(secretspec::SecretSpecSecrets::new(
                        data,
                        validation_result.provider,
                        validation_result.profile
                    ))
                }
            }
        }
    }

    /// Generate all builder-related code
    pub fn generate_all(
        load_assignments: &[proc_macro2::TokenStream],
        load_profile_arms: &[proc_macro2::TokenStream],
        first_profile_variant: &proc_macro2::Ident,
    ) -> proc_macro2::TokenStream {
        let struct_def = generate_struct();
        let basic_methods = generate_basic_methods();
        let load_methods =
            generate_load_methods(load_assignments, load_profile_arms, first_profile_variant);

        quote! {
            #struct_def
            #basic_methods
            #load_methods
        }
    }
}

/// Main code generation function
fn generate_secret_spec_code(config: ProjectConfig) -> proc_macro2::TokenStream {
    // Collect all profiles
    let all_profiles: HashSet<String> = config.profiles.keys().cloned().collect();
    let profile_variants = get_profile_variants(&all_profiles);

    // Analyze field types
    let field_info = analyze_field_types(&config);

    // Generate field assignments for load()
    let load_assignments: Vec<_> = field_info
        .values()
        .map(|info| info.generate_assignment(quote! { secrets }))
        .collect();

    // Generate env var setters
    let env_setters: Vec<_> = field_info
        .values()
        .map(|info| info.generate_env_setter())
        .collect();

    // Generate profile components
    let profile_code = profile_generation::generate_all(&profile_variants);

    // Generate SecretSpec components
    let secret_spec_struct = secret_spec_generation::generate_struct(&field_info);
    let profile_enum_variants = secret_spec_generation::generate_profile_enum_variants(
        &config,
        &field_info,
        &profile_variants,
    );
    let secret_spec_profile_enum =
        secret_spec_generation::generate_profile_enum(&profile_enum_variants);
    let load_profile_arms =
        secret_spec_generation::generate_load_profile_arms(&config, &field_info, &profile_variants);
    let load_internal = secret_spec_generation::generate_load_internal();
    let secret_spec_impl = secret_spec_generation::generate_impl(&load_assignments, env_setters);

    // Get first profile variant for defaults
    // Get first profile variant for defaults
    let first_profile_variant = profile_variants
        .first()
        .map(|v| v.as_ident())
        .unwrap_or_else(|| format_ident!("Default"));

    // Generate builder
    let builder_code = builder_generation::generate_all(
        &load_assignments,
        &load_profile_arms,
        &first_profile_variant,
    );

    // Combine all components
    quote! {
        #secret_spec_struct
        #secret_spec_profile_enum
        #profile_code

        // Use Provider from secretspec_types
        pub use secretspec::Provider;

        // Type alias to help with type inference
        type LoadResult<T> = Result<T, secretspec::SecretSpecError>;

        #load_internal
        #builder_code
        #secret_spec_impl
    }
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
