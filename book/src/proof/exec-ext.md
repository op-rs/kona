# `kona-executor` Extensions

The `kona-executor` crate offers a to-spec, stateless implementation of the OP Stack STF. However, due to the
power of [`revm`][revm]'s Handler abstractions, the logic of the STF can be easily modified.

To register a custom handler, for example to add a custom precompile, modify the behavior of an EVM opcode,
or change the fee handling, `StatelessL2BlockExecutorBuilder::with_handle_register` is your friend. It accepts a
[`KonaHandleRegister`](https://docs.rs/kona-executor/latest/kona_executor/type.KonaHandleRegister.html), which
can be used to take full advantage of [`revm`'s Handler API](https://github.com/bluealloy/revm/blob/f57e3e639ee157c7e659e740bd175a7357003570/documentation/src/crates/revm/handler.md#handler).

## Example - Custom Precompile

```rs
const MY_PRECOMPILE_ADDRESS: Address = u64_to_address(0xFF);

fn my_precompile(input: &Bytes, gas_limit: u64) -> PrecompileResult {
   Ok(PrecompileOutput::new(50, "hello, world!".as_bytes().into()))
}

fn custom_handle_register<F, H>(
    handler: &mut EvmHandler<'_, (), &mut State<&mut TrieDB<F, H>>>,
) where
   F: TrieProvider,
   H: TrieHinter,
{
   let spec_id = handler.cfg.spec_id;

   handler.pre_execution.load_precompiles = Arc::new(move || {
      let mut ctx_precompiles = spec_to_generic!(spec_id, {
         revm::optimism::load_precompiles::<SPEC, (), &mut State<&mut TrieDB<F, H>>>()
      });

      let precompile = PrecompileWithAddress(
         MY_PRECOMPILE_ADDRESS,
         Precompile::Standard(my_precompile)
      );
      ctx_precompiles.extend([precompile]);

      ctx_precompiles
   });
}

// - snip -

let cfg = RollupConfig::default();
let provider = ...;
let hinter = ...;

let executor = StatelessL2BlockExecutor::builder(&cfg, provider, hinter)
   .with_parent_header(...)
   .with_handle_register(custom_handle_register)
   .build();
```

{{ #include ../links.md }}
