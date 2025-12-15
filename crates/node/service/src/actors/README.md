
# Actors

The system comprises the following actors and connections

```mermaid
    flowchart TD

    seq[SequencerActor]
    eng[EngineActor]
    l1w[L1WatcherActor]
    net[NetworkActor]
    rpc[RPCActor]
    der[DerivationActor]
    adm{Admin RPC}


    l1w -- channel_name --> seq
    eng -- channel_name --> seq
    rpc --> der
    rpc --> seq
    net --> seq
    adm -- admin_tx --> seq
    seq -- admin_rx --> adm
```