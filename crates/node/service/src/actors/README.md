
# Actors

The system comprises the following actors and connected through channels

```mermaid
    flowchart TD

    adm{Admin RPC}
    seq[Sequencer]
    eng[Engine]
    l1w[L1Watcher]
    net[Network]
    rpc[RPC]
    der[Derivation]


    adm -- admin_tx --> seq
    seq -- admin_rx --> adm
    l1w -- channel_name --> seq
    eng -- channel_name --> seq
    rpc --> der --> net --> seq
```