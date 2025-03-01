# Kona Node

These docs walk through kona's rollup (aka consensus) node.

### Learned Principles

One of the key design principles of the Kona Node is to apply
learnings from the reference [op-node][op-node]. These include
the following.
* **Directional Messaging**: Traceability and legibility depend on directional, clear message passing.
* **Message Passing over Shared State**: Communicate between and schedule tasks for components of the node through messaging instead of sharing state.
