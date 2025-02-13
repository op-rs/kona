# Kona Monorepo Structure

This is a living document that outlines the structure of the `kona` monorepo.

```
crates/
  proof/
    mpt/
    executor/
    preimage/
    fpvm/
    fpvm-proc/
    proof
    proof-interop/
  protocol/
    derive/
    driver/
    interop/
    genesis/   <-- Has Maili Shadow
    registry/  <-- Has Maili Shadow
    protocol/  <-- Has Maili Shadow
  services/
    rpc/       <-- Has Maili Shadow
    net/
    providers-alloy/
    providers-local/
  utilities/
    serde/     <-- Has Maili Shadow
  cli/
    ..         # TODO
```
