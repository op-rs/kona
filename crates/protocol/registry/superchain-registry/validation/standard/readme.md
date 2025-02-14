# Standard Rollup Blockspace Charter definition files
This directory contains a number of TOML files which declare various parameters, addresses, contract versions and other data which define a **standard chain** in the sense of the [Standard Rollup Charter](https://gov.optimism.io/t/season-6-draft-standard-rollup-charter/8135).

Distinct, named declaration files have been added (where necessary) to allow testnets to have greater flexibility in their configurations.

Parameters may be declared to be equal to a pair of values, meaning that the parameter must be within the bounds defined by those values.

The TOML files are embedded into Go bindings, which are in turn referenced by the validation checks in the parent directory. The entrypoint for those checks is [`validation_test.go`](../validation_test.go).

