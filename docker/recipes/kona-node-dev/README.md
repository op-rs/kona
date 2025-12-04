# Requirements

This is intended to run on x86-64 architecture.

# Kona node dev environment

Assuming you are on Ubuntu and your user is member of the group `docker`:

First time run

    git clone 'git clone git@github.com:op-rs/kona.git'
    mv kona/docker/recipes/kona-node-dev/ .
    cd kona-node-dev
    just init

If the last step fails due to missing packages, you can run `just setup-ubuntu`
and then `just init` again.  This will install the required packages for
Ubuntu.  `just init` will also set up a virtual network, and finally spin up `kona-node` and
`op-reth`.

For future invocation it suffices to spin `up` and `down` with:

    just up
    just down

You can also run `just upd` if you want to detach from the docker logs. 
If you want to update the `kona` submodule, you can run `just update`.

A typical workflow after init could look like this:

    # remove existing images so that they will rebuild
    just rmi 
    # maybe update submodules
    just update
    # checkout dev branch
    just checkout my-username/my-branch 
    # build images and start containers
    just upd
    # visit Grafana
    just stop

For more info on the commands please refer to `justfile`.

# Environment
    
To use different RPC servers, you can copy the variables you want to override
from  `op-rs-box1.env` in to `override.env` and set your own values.
Alternatively, you can define your own `.env` and point to it in
`docker-compose.yml`.


# Services and observability

The following services are provided:

    http://localhost:3000

Default credentials are admin:admin and you should change that if you plan to use this instance over longer time.


# Storage

The data is stored in current directory `./datadirs`, but you can modify the `volume` mapping in `docker-compose.yml` to use a different directory.


# Bugs and development

Everything is orchestrated from `justfile`.  Feel free to edit and submit PRs.
