extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, ItemFn, LitInt, Result,
};

/// The arguments for the `#[client_entry]` attribute proc macro
struct MacroArgs {
    /// The heap size to allocate
    heap_size: LitInt,
}

impl Parse for MacroArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let heap_size = input.parse()?;
        Ok(MacroArgs { heap_size })
    }
}

#[proc_macro_attribute]
pub fn client_entry(attr: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as MacroArgs);
    let input_fn = parse_macro_input!(input as ItemFn);

    let heap_size = args.heap_size;
    let fn_body = &input_fn.block;
    let fn_name = &input_fn.sig.ident;

    let expanded = quote! {
        fn #fn_name() -> Result<(), String> {
            match #fn_body {
                Ok(_) => kona_std_fpvm::io::exit(0),
                Err(e) => {
                    kona_std_fpvm::io::print_err(alloc::format!("Program encountered fatal error: {:?}\n", e).as_ref());
                    kona_std_fpvm::io::exit(1);
                }
            }
        }

        cfg_if::cfg_if! {
            if #[cfg(any(target_arch = "mips64", target_arch = "riscv64"))] {
                const HEAP_SIZE: usize = #heap_size;

                #[doc = "Program entry point"]
                #[no_mangle]
                pub extern "C" fn _start() {
                    kona_std_fpvm::alloc_heap!(HEAP_SIZE);
                    let _ = #fn_name();
                }

                #[panic_handler]
                fn panic(info: &core::panic::PanicInfo) -> ! {
                    let msg = alloc::format!("Panic: {}", info);
                    kona_std_fpvm::io::print_err(msg.as_ref());
                    kona_std_fpvm::io::exit(2)
                }
            }
        }
    };

    TokenStream::from(expanded)
}
