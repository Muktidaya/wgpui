use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

/// Resolve the framework API exposed to the crate where a macro is expanded.
///
/// Canonical WGPUI packages come first. `proc_macro_crate` resolves their actual
/// Cargo dependency keys, including downstream renames. The remaining names are
/// legacy external-package compatibility probes, not workspace import aliases;
/// they do not establish full upstream-platform compatibility.
pub(crate) fn wgpui() -> syn::Result<TokenStream> {
    for name in ["wgpui", "wgpui-kit", "gpui", "gpui-kit", "gpui-pre"] {
        if let Ok(found) = crate_name(name) {
            return Ok(found_crate_path(found));
        }
    }
    Err(syn::Error::new(
        Span::call_site(),
        "IntoPlot requires a direct dependency on `wgpui`, `wgpui-kit`, `gpui`, `gpui-kit`, or `gpui-pre`",
    ))
}

fn found_crate_path(found: FoundCrate) -> TokenStream {
    match found {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
    }
}

/// Resolve the styled plot API, including renamed Kit dependencies.
pub(crate) fn component() -> syn::Result<TokenStream> {
    for name in ["wgpui-kit", "gpui-kit"] {
        if let Ok(found) = crate_name(name) {
            let kit = found_crate_path(found);
            return Ok(quote!(#kit::component));
        }
    }
    for name in ["wgpui-component", "gpui-component"] {
        if let Ok(found) = crate_name(name) {
            return Ok(found_crate_path(found));
        }
    }
    Err(syn::Error::new(
        Span::call_site(),
        "IntoPlot requires wgpui-kit or wgpui-component",
    ))
}
