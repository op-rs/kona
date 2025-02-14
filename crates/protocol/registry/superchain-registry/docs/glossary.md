# Glossary

To help with clarity and a common understanding, here are some helpful terms and their definitions.

### General

* **Superchain Ecosystem member:** A chain with an agreement in place to commit sequencer revenue back to the Optimism Collective.
* **Blockspace charter:** A [Blockspace Charter](https://gov.optimism.io/t/season-6-introducing-blockspace-charters-superchain-first-governance/8133) is a technical-focused governing document (and framework) for the Superchain.
* **Standard chain:** A chain that conforms to the [Standard Rollup Charter](https://gov.optimism.io/t/season-6-draft-standard-rollup-charter/8135)
* **Frontier chain:** A non-standard chain that has modifications that do not fit the `Standard Rollup Charter` criteria.
* **Standard chain candidate:** A chain that has met most of the standard chain criteria, except for the `ProxyAdminOwner` key handover.
* **Key handover:**  A colloquial term for updating the chain's `ProxyAdminOwner` to fulfill the requirements of the standard rollup charter.

### Superchain Level and Rollup Stage

Chains in the superchain-registry are assigned a `superchain_level` (shown in individual config files as well as the `chainList.toml/json` summaries), depending on the set of validation checks that they pass.

**Non-Standard** chains have `superchain_level = 0`. These are members of the Superchain ecosystem.

**Standard Candidate** chains have `superchain_level = 1`. These are chains who conform to the standard blockchain charter, but haven't handed over their keys yet.

**Standard** chains have `superchain_level = 2`. These chains conform to the standard blockchain charter, _and_ have handed over their keys.
