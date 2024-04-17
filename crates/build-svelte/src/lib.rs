use proc_macro::TokenStream;

#[proc_macro]
pub fn build_svelte(_item: TokenStream) -> TokenStream {
    TokenStream::default()
}
