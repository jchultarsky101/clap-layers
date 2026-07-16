//! Proc-macro implementation for the [`clap-layers`](https://docs.rs/clap-layers) crate.
//!
//! This crate is an implementation detail; use `clap_layers::Layered`, which
//! re-exports the derive alongside the trait it implements.
//!
//! # What the derive generates
//!
//! Given:
//!
//! ```ignore
//! #[derive(Parser, Layered)]
//! #[layered(file = "config.toml", env_prefix = "MYAPP")]
//! struct Config {
//!     #[arg(long, default_value_t = 3000)]
//!     port: u16,
//! }
//! ```
//!
//! the derive emits `Layered::layered_from`, which:
//!
//! 1. builds clap's `Command` and parses the arguments into `ArgMatches`,
//! 2. loads the TOML file **once**,
//! 3. resolves each field via `clap_layers::__private::resolve`, using clap's
//!    `ValueSource` to tell a flag the user actually typed from one clap
//!    defaulted.
//!
//! The merge logic itself lives in the runtime crate, so the expansion stays
//! small and the engine is tested once rather than re-emitted per field.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
// A library returns `Result`; it must not abort its caller's process. Relaxed
// under `cfg(test)`, where unwrapping is how a test asserts.
#![cfg_attr(
    not(test),
    warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::ext::IdentExt as _;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned as _;
use syn::{Attribute, DeriveInput, Expr, ExprLit, Field, Lit, LitStr, Meta, Token};

/// Derive the `Layered` trait, giving a clap struct layered configuration.
///
/// See the [crate-level documentation](crate) and the
/// [`clap-layers` docs](https://docs.rs/clap-layers) for the attribute grammar.
#[proc_macro_derive(Layered, attributes(layered))]
pub fn derive_layered(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Struct-level `#[layered(...)]` options.
#[derive(Default)]
struct ContainerAttrs {
    file: Option<LitStr>,
    env_prefix: Option<LitStr>,
}

/// Field-level `#[layered(...)]` markers, plus what clap does with the field.
#[derive(Default)]
struct FieldAttrs {
    no_cli: bool,
    no_file: bool,
    no_env: bool,
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let container = parse_container_attrs(&input.attrs)?;

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(named) => &named.named,
            syn::Fields::Unnamed(_) => {
                return Err(syn::Error::new_spanned(
                    name,
                    "`Layered` does not support tuple structs\n  \
                     help: layering resolves values by field name, so fields must be named",
                ));
            }
            syn::Fields::Unit => {
                return Err(syn::Error::new_spanned(
                    name,
                    "`Layered` does not support unit structs\n  \
                     help: there is nothing to configure on a struct with no fields",
                ));
            }
        },
        syn::Data::Enum(_) => {
            return Err(syn::Error::new_spanned(
                name,
                "`Layered` does not support enums\n  \
                 help: derive `Layered` on your top-level `Parser` struct. \
                 Subcommand enums are not supported yet",
            ));
        }
        syn::Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                name,
                "`Layered` does not support unions",
            ));
        }
    };

    // Collect every field error before bailing, so one build surfaces them all.
    let mut errors: Option<syn::Error> = None;
    let mut inits = Vec::new();
    for field in fields {
        match field_init(field, &container) {
            Ok(init) => inits.push(init),
            Err(e) => match &mut errors {
                Some(acc) => acc.combine(e),
                None => errors = Some(e),
            },
        }
    }
    if let Some(e) = errors {
        return Err(e);
    }

    let file_expr = quote_option(container.file.as_ref());

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::clap_layers::Layered for #name #ty_generics #where_clause {
            fn layered_from<__I, __T>(
                args: __I,
                env: &::clap_layers::Env,
            ) -> ::core::result::Result<Self, ::clap_layers::LayeredError>
            where
                __I: ::core::iter::IntoIterator<Item = __T>,
                __T: ::core::convert::Into<::std::ffi::OsString> + ::core::clone::Clone,
            {
                let __matches = <Self as ::clap_layers::clap::CommandFactory>::command()
                    .try_get_matches_from(args)?;
                let __cli = <Self as ::clap_layers::clap::FromArgMatches>::from_arg_matches(&__matches)?;

                // Read and parse the config file exactly once, not per field.
                let __file = ::clap_layers::__private::load_file(#file_expr)?;
                let __file = __file.as_ref();

                ::core::result::Result::Ok(Self {
                    #(#inits),*
                })
            }
        }
    })
}

/// Build the initializer for one field.
fn field_init(field: &Field, container: &ContainerAttrs) -> syn::Result<TokenStream2> {
    let ident = field
        .ident
        .as_ref()
        .ok_or_else(|| syn::Error::new_spanned(field, "expected a named field"))?;

    // clap owns flattened and subcommand fields entirely; they have no argument
    // id, so asking for their `ValueSource` would panic. Pass them through.
    if has_clap_marker(field, &["command", "clap"], "flatten")
        || has_clap_marker(field, &["command", "clap", "arg"], "subcommand")
    {
        return Ok(quote!(#ident: __cli.#ident));
    }

    let attrs = parse_field_attrs(&field.attrs)?;
    let skipped = has_clap_marker(field, &["arg", "clap"], "skip");

    if attrs.no_cli && !skipped {
        return Err(syn::Error::new_spanned(
            field,
            "`#[layered(no_cli)]` also requires `#[arg(skip)]` on the same field\n  \
             help: without `#[arg(skip)]` clap still defines a flag for this field, so it \
             would not actually be hidden from the command line\n  \
             note: `#[arg(skip)]` requires the field type to implement `Default`",
        ));
    }

    // Strip any `r#` prefix: a field written `r#type` is the argument id
    // `type` to clap, and the key `type` in a config file. Using the raw form
    // here would query an id clap never registered, which panics in debug
    // builds. `#ident` below deliberately keeps the raw form, since that is
    // what the field is actually called in Rust.
    let field_name = ident.unraw().to_string();

    // `no_cli` and `#[arg(skip)]` both mean clap never sees this field.
    let env_var = match (&container.env_prefix, attrs.no_env) {
        (Some(prefix), false) => Some(format!(
            "{}_{}",
            prefix.value().to_uppercase(),
            field_name.to_uppercase()
        )),
        _ => None,
    };

    if skipped && env_var.is_none() && (attrs.no_file || container.file.is_none()) {
        return Err(syn::Error::new_spanned(
            field,
            "this field has no configuration source\n  \
             help: it is hidden from the command line, and the environment and file layers \
             are unavailable to it, so it could only ever hold `Default::default()`",
        ));
    }

    let env_expr = quote_option(env_var.as_ref());
    let file_expr = if attrs.no_file {
        quote!(::core::option::Option::None)
    } else {
        quote!(__file)
    };
    let explicit_expr = if skipped {
        // Not a clap argument, so it can never have been typed.
        quote!(false)
    } else {
        let id = clap_arg_id(field, &field_name);
        quote!(::clap_layers::__private::is_explicit(&__matches, #id))
    };

    Ok(quote! {
        #ident: ::clap_layers::__private::resolve(
            #field_name,
            #env_expr,
            env,
            #file_expr,
            #explicit_expr,
            __cli.#ident,
        )?
    })
}

/// Quote an `Option<T>` as a literal `Option` expression in the output.
fn quote_option<T: quote::ToTokens>(value: Option<&T>) -> TokenStream2 {
    value.map_or_else(
        || quote!(::core::option::Option::None),
        |value| quote!(::core::option::Option::Some(#value)),
    )
}

/// Parse `#[layered(file = "...", env_prefix = "...")]` on the struct.
fn parse_container_attrs(attrs: &[Attribute]) -> syn::Result<ContainerAttrs> {
    let mut out = ContainerAttrs::default();

    for meta in layered_metas(attrs)? {
        match ident_of(&meta).as_deref() {
            Some("file") => {
                let lit = expect_string(&meta, "file", "myapp.toml")?;
                if lit.value().is_empty() {
                    return Err(syn::Error::new_spanned(
                        &lit,
                        "`file` must not be empty\n  \
                         help: an empty path names no file, so the layer would silently \
                         never apply. Remove `file` to disable the layer deliberately",
                    ));
                }
                set_once(&mut out.file, lit, &meta, "file")?;
            }
            Some("env_prefix") => {
                let lit = expect_string(&meta, "env_prefix", "MYAPP")?;
                validate_env_prefix(&lit)?;
                set_once(&mut out.env_prefix, lit, &meta, "env_prefix")?;
            }
            other => {
                return Err(unknown_option(
                    &meta,
                    other,
                    &["file", "env_prefix"],
                    "struct",
                ));
            }
        }
    }

    Ok(out)
}

/// Parse `#[layered(no_cli, no_file, no_env)]` on a field.
fn parse_field_attrs(attrs: &[Attribute]) -> syn::Result<FieldAttrs> {
    let mut out = FieldAttrs::default();

    for meta in layered_metas(attrs)? {
        let name = ident_of(&meta);
        let flag = match name.as_deref() {
            Some("no_cli") => &mut out.no_cli,
            Some("no_file") => &mut out.no_file,
            Some("no_env") => &mut out.no_env,
            other => {
                return Err(unknown_option(
                    &meta,
                    other,
                    &["no_cli", "no_file", "no_env"],
                    "field",
                ));
            }
        };

        // These are markers: `#[layered(no_env)]`, never `#[layered(no_env = ...)]`.
        if !matches!(meta, Meta::Path(_)) {
            let name = name.unwrap_or_default();
            return Err(syn::Error::new_spanned(
                &meta,
                format!("`{name}` does not take a value\n  help: write `#[layered({name})]`"),
            ));
        }
        if *flag {
            let name = name.unwrap_or_default();
            return Err(syn::Error::new_spanned(
                &meta,
                format!("`{name}` is specified more than once"),
            ));
        }
        *flag = true;
    }

    Ok(out)
}

/// Flatten every `#[layered(...)]` attribute into its comma-separated items.
fn layered_metas(attrs: &[Attribute]) -> syn::Result<Vec<Meta>> {
    let mut out = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("layered")) {
        if matches!(attr.meta, Meta::Path(_)) {
            return Err(syn::Error::new_spanned(
                attr,
                "`#[layered]` requires options\n  \
                 help: write `#[layered(file = \"myapp.toml\")]`, or remove the attribute",
            ));
        }
        out.extend(attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?);
    }
    Ok(out)
}

fn ident_of(meta: &Meta) -> Option<String> {
    meta.path().get_ident().map(ToString::to_string)
}

/// Reject a prefix that would build environment variable names nobody can set.
///
/// `env_prefix = "my-app"` yields `MY-APP_PORT`, which most shells cannot
/// export, so the layer would appear to do nothing at runtime. Better to say so
/// at compile time.
fn validate_env_prefix(lit: &LitStr) -> syn::Result<()> {
    let value = lit.value();

    if value.is_empty() {
        return Err(syn::Error::new_spanned(
            lit,
            "`env_prefix` must not be empty\n  \
             help: an empty prefix would map a field named `path` to the ambient `PATH`",
        ));
    }

    if let Some(bad) = value
        .chars()
        .find(|c| !c.is_ascii_alphanumeric() && *c != '_')
    {
        return Err(syn::Error::new_spanned(
            lit,
            format!(
                "`env_prefix` contains `{bad}`, which is not valid in an environment \
                 variable name\n  \
                 help: use ASCII letters, digits and underscores, as in \
                 `#[layered(env_prefix = \"MY_APP\")]`"
            ),
        ));
    }

    if value.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(syn::Error::new_spanned(
            lit,
            "`env_prefix` must not start with a digit\n  \
             help: an environment variable name cannot begin with a digit",
        ));
    }

    Ok(())
}

/// Look through the invisible groups that `macro_rules!` wraps around a
/// substituted metavariable.
///
/// A struct generated by a `macro_rules!` macro passes `$file:literal` through
/// as an `Expr::Group`, not a bare `Expr::Lit`. Matching only the bare form
/// would reject a perfectly good string literal with "must be a string
/// literal", which is a baffling error to receive.
fn peel_groups(mut expr: &Expr) -> &Expr {
    while let Expr::Group(group) = expr {
        expr = &group.expr;
    }
    expr
}

/// Require `name = "..."` and hand back the literal.
fn expect_string(meta: &Meta, name: &str, example: &str) -> syn::Result<LitStr> {
    match meta {
        Meta::NameValue(nv) => match peel_groups(&nv.value) {
            Expr::Lit(ExprLit {
                lit: Lit::Str(lit), ..
            }) => Ok(lit.clone()),
            other => Err(syn::Error::new_spanned(
                other,
                format!(
                    "`{name}` must be a string literal\n  help: write `#[layered({name} = \"{example}\")]`"
                ),
            )),
        },
        other => Err(syn::Error::new_spanned(
            other,
            format!(
                "`{name}` requires a value\n  help: write `#[layered({name} = \"{example}\")]`"
            ),
        )),
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T, meta: &Meta, name: &str) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new_spanned(
            meta,
            format!("`{name}` is specified more than once"),
        ));
    }
    *slot = Some(value);
    Ok(())
}

fn unknown_option(meta: &Meta, found: Option<&str>, supported: &[&str], on: &str) -> syn::Error {
    let found = found.unwrap_or("<non-identifier>");
    let supported = supported
        .iter()
        .map(|s| format!("`{s}`"))
        .collect::<Vec<_>>()
        .join(", ");
    syn::Error::new(
        meta.span(),
        format!(
            "unknown `layered` option `{found}`\n  \
             help: supported options on a {on} are: {supported}"
        ),
    )
}

/// Does the field carry a bare clap marker such as `#[arg(skip)]` or
/// `#[command(flatten)]`?
fn has_clap_marker(field: &Field, attr_names: &[&str], marker: &str) -> bool {
    field
        .attrs
        .iter()
        .filter(|attr| attr_names.iter().any(|n| attr.path().is_ident(n)))
        .any(|attr| {
            attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .is_ok_and(|metas| metas.iter().any(|m| m.path().is_ident(marker)))
        })
}

/// The id clap registers this argument under: the field name, unless the user
/// overrode it with `#[arg(id = "...")]`.
fn clap_arg_id(field: &Field, field_name: &str) -> String {
    for attr in field
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("arg") || a.path().is_ident("clap"))
    {
        let Ok(metas) = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        else {
            continue;
        };
        for meta in metas {
            if !meta.path().is_ident("id") {
                continue;
            }
            if let Meta::NameValue(nv) = &meta {
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(lit), ..
                }) = peel_groups(&nv.value)
                {
                    return lit.value();
                }
            }
        }
    }
    field_name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run the derive over a struct written as source text.
    ///
    /// `expand` takes `syn`/`proc_macro2` types rather than the compiler's own
    /// `proc_macro::TokenStream`, so the whole macro is testable in-process.
    /// `trybuild` pins how these errors *render*; these tests pin what they
    /// *say*, and run in milliseconds rather than invoking rustc.
    fn expand_str(source: &str) -> syn::Result<TokenStream2> {
        let input: DeriveInput = syn::parse_str(source).expect("test input should parse");
        expand(&input)
    }

    /// Every error message from expanding `source`, which must fail.
    ///
    /// `syn::Error::to_string` reports only the first of a combined error, so
    /// iterate to see them all — otherwise a test asserting on the second
    /// message would pass for the wrong reason.
    fn error(source: &str) -> String {
        match expand_str(source) {
            Ok(_) => panic!("expected an error, but the derive succeeded"),
            Err(e) => e
                .into_iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n---\n"),
        }
    }

    /// The generated code as a string, for asserting on what was emitted.
    fn generated(source: &str) -> String {
        expand_str(source)
            .expect("expected the derive to succeed")
            .to_string()
    }

    #[test]
    fn rejects_shapes_it_cannot_support() {
        assert!(error("enum E { A, B }").contains("does not support enums"));
        assert!(error("union U { a: u16 }").contains("does not support unions"));
        assert!(error("struct S(u16);").contains("does not support tuple structs"));
        assert!(error("struct S;").contains("does not support unit structs"));
    }

    #[test]
    fn rejects_unknown_options_rather_than_ignoring_them() {
        // A silently ignored option is how `no_env` came to do nothing at all.
        let err = error(r#"#[layered(fil = "a.toml")] struct S { a: u16 }"#);
        assert!(err.contains("unknown `layered` option `fil`"), "{err}");
        assert!(
            err.contains("`file`"),
            "the help must list what is valid: {err}"
        );

        let err = error(r"struct S { #[layered(no_envv)] a: u16 }");
        assert!(err.contains("unknown `layered` option `no_envv`"), "{err}");
        assert!(err.contains("`no_env`"), "{err}");
    }

    #[test]
    fn rejects_malformed_options() {
        assert!(error(r"#[layered(file)] struct S { a: u16 }").contains("`file` requires a value"));
        assert!(
            error(r"#[layered(file = 1)] struct S { a: u16 }")
                .contains("`file` must be a string literal")
        );
        assert!(
            error(r#"struct S { #[layered(no_env = "true")] a: u16 }"#)
                .contains("`no_env` does not take a value")
        );
        assert!(error(r"#[layered] struct S { a: u16 }").contains("`#[layered]` requires options"));
    }

    #[test]
    fn rejects_duplicate_options() {
        assert!(
            error(r#"#[layered(file = "a.toml", file = "b.toml")] struct S { a: u16 }"#)
                .contains("`file` is specified more than once")
        );
        assert!(
            error(r"struct S { #[layered(no_env, no_env)] a: u16 }")
                .contains("`no_env` is specified more than once")
        );
    }

    #[test]
    fn rejects_an_env_prefix_that_cannot_name_a_variable() {
        assert!(
            error(r#"#[layered(env_prefix = "")] struct S { a: u16 }"#)
                .contains("must not be empty")
        );
        assert!(
            error(r#"#[layered(env_prefix = "my-app")] struct S { a: u16 }"#)
                .contains("contains `-`")
        );
        assert!(
            error(r#"#[layered(env_prefix = "my app")] struct S { a: u16 }"#)
                .contains("not valid in an environment variable name")
        );
        assert!(
            error(r#"#[layered(env_prefix = "1app")] struct S { a: u16 }"#)
                .contains("must not start with a digit")
        );
    }

    #[test]
    fn rejects_an_empty_file_path() {
        assert!(
            error(r#"#[layered(file = "")] struct S { a: u16 }"#)
                .contains("`file` must not be empty")
        );
    }

    #[test]
    fn requires_arg_skip_alongside_no_cli() {
        let err = error(r"struct S { #[layered(no_cli)] a: u16 }");
        assert!(err.contains("requires `#[arg(skip)]`"), "{err}");

        // With `#[arg(skip)]` and a layer to read from, it is accepted.
        assert!(
            expand_str(r#"#[layered(env_prefix = "A")] struct S { #[layered(no_cli)] #[arg(skip)] a: u16 }"#)
                .is_ok()
        );
    }

    #[test]
    fn rejects_a_field_no_layer_can_reach() {
        let err = error(r"struct S { #[layered(no_cli)] #[arg(skip)] a: u16 }");
        assert!(err.contains("no configuration source"), "{err}");
    }

    #[test]
    fn reports_every_bad_field_in_one_pass() {
        // Stopping at the first error would make fixing a struct a slow loop.
        let err = error(
            r#"#[layered(env_prefix = "A")] struct S {
                #[layered(no_envv)] a: u16,
                #[layered(no_cli)] b: u16,
            }"#,
        );
        assert!(err.contains("no_envv"), "{err}");
        assert!(err.contains("requires `#[arg(skip)]`"), "{err}");
    }

    #[test]
    fn environment_variable_names_are_prefixed_and_uppercased() {
        let code = generated(r#"#[layered(env_prefix = "myapp")] struct S { db_password: u16 }"#);
        assert!(code.contains("\"MYAPP_DB_PASSWORD\""), "{code}");
    }

    #[test]
    fn the_environment_layer_is_off_without_a_prefix() {
        // Otherwise a field named `path` would read the ambient PATH.
        let code = generated(r"struct S { path: String }");
        assert!(!code.contains("\"PATH\""), "{code}");
    }

    #[test]
    fn markers_drop_the_layer_they_name() {
        let with_env = generated(r#"#[layered(env_prefix = "A")] struct S { a: u16 }"#);
        assert!(with_env.contains("\"A_A\""));

        let no_env =
            generated(r#"#[layered(env_prefix = "A")] struct S { #[layered(no_env)] a: u16 }"#);
        assert!(
            !no_env.contains("\"A_A\""),
            "no_env must not emit a variable name"
        );
    }

    #[test]
    fn raw_identifiers_are_unraw_for_clap_and_the_file_but_not_the_field() {
        let code = generated(r#"#[layered(env_prefix = "A")] struct S { r#type: u16 }"#);

        // The key, the id and the variable use the unraw name...
        assert!(code.contains(r#""type""#), "{code}");
        assert!(code.contains(r#""A_TYPE""#), "{code}");
        // ...while the field itself keeps its raw spelling.
        assert!(code.contains("r#type"), "{code}");
    }

    #[test]
    fn a_skipped_field_never_queries_value_source() {
        // `ArgMatches::value_source` panics in debug builds for an unknown id.
        let skipped = generated(r#"#[layered(env_prefix = "A")] struct S { #[arg(skip)] a: u16 }"#);
        assert!(!skipped.contains("is_explicit"), "{skipped}");

        let normal = generated(r#"#[layered(env_prefix = "A")] struct S { a: u16 }"#);
        assert!(normal.contains("is_explicit"), "{normal}");
    }

    #[test]
    fn flattened_and_subcommand_fields_are_passed_through() {
        for source in [
            r"struct S { #[command(flatten)] a: Nested }",
            r"struct S { #[command(subcommand)] a: Cmd }",
            r"struct S { #[clap(flatten)] a: Nested }",
        ] {
            let code = generated(source);
            assert!(
                !code.contains("is_explicit") && !code.contains("resolve"),
                "{source} should pass through untouched, got: {code}"
            );
        }
    }

    #[test]
    fn a_custom_arg_id_is_used_for_value_source() {
        let code = generated(r#"struct S { #[arg(id = "renamed", long = "port")] port: u16 }"#);
        assert!(code.contains(r#""renamed""#), "{code}");
    }

    #[test]
    fn the_file_is_loaded_once_rather_than_per_field() {
        let code = generated(r#"#[layered(file = "c.toml")] struct S { a: u16, b: u16, c: u16 }"#);
        assert_eq!(
            code.matches("load_file").count(),
            1,
            "the file must be read once per parse, not once per field: {code}"
        );
    }

    #[test]
    fn generics_and_where_clauses_are_carried_onto_the_impl() {
        let code = generated(r"struct S<T: Clone> where T: Send { a: u16, b: T }");
        assert!(code.contains("impl < T : Clone >"), "{code}");
        assert!(code.contains("where T : Send"), "{code}");
    }
}
