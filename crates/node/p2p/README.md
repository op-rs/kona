# `kona-p2p`

A p2p library for the OP Stack.

Contains a gossipsub driver to run discv5 peer discovery and block gossip.


### Technical note:

Contrarily to the `op-node`, `kona-node`s don't manually track peer scores. For simplicity, we're relying on the peer score computed by `rust-libp2p`. Since this library doesn't expose all the factors used to compute the peer score (like per topic scores, or the ip-collocation-factor), we're only exposing the total peer score.

See `<https://github.com/libp2p/rust-libp2p/issues/6058>`

### Acknowledgements

Largely based off [magi]'s [p2p module][p2p].

<!-- Links -->

[magi]: https://github.com/a16z/magi
[p2p]: https://github.com/a16z/magi/tree/master/src/network
