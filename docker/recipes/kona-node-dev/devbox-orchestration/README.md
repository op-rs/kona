This file is WIP.

# Kona node dev environment

Assuming you are on Ubuntu and your user is member of the group `docker`:

First time run

    git clone 'git@github.com-oplabs:einar-oplabs/op-rs-ops.git'
    just init

If the last step fails due to missing packages, you can run `just setup-ubuntu`
and then `just init` again.  This will install the required packages for
Ubuntu.  `just init` will also set up a virtual network, and finally spin up `kona-node` and
`op-reth`.

For future invocation it suffices to spin `up` and `down` with:

    just up
    just down


# Environment
    
    To use different RPC servers, you can copy the variables you want to
    override from  `op-rs-box1.env` in to `override.env` and set your own values.
    Alternatively, you can define your own `.env` and point to it in `docker-compose.yml`.


# Services and observability

The following services are provided:

    localhost://ops-rs-op-sepolia


# Storage

The data is stored in current directory `./datadirs`, but you can modify the `volume` mapping in `docker-compose.yml` to use a different directory.


# Bugs and development

Everything is orchestrated from `justfile`.  Feel free to edit and submit PRs.
