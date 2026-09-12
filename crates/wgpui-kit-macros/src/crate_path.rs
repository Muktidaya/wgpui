use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

/// Resolve the GPUI API exposed to the crate where a macro is expanded.
///
/// Order: `wgpui` (this adaptor), then `wgpui-kit` and the import alias `gpui`, then
/// upstream `gpui-kit` / `gpui-pre` so a later kit consumer still resolves.
pub(crate) fn gpui() -> syn::Result<TokenStream> {
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
