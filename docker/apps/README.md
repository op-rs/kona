# `docker-apps`

This directory contains a generic dockerfile that accepts two arguments: `TAG` and `BIN_TARGET`. It clones the `kona`
repository at the specified `TAG`, and builds the `BIN_TARGET` with the `release` profile, before finally copying it
into an `ubuntu:22.04` image with the binary in the PATH and set as the image's entrypoint.

Adding a new binary to the `docker-bake.hcl` in the parent directory is as follows:

```hcl
target "<bin-name>" {
  inherits = ["docker-metadata-action"]
  context = "."
  dockerfile = "docker/apps/kona_app_generic.dockerfile"
  args = {
    TAG = "${GIT_REF_NAME}"
    BIN_TARGET = "<bin-name>"
  }
  platforms = split(",", PLATFORMS)
}
```

From there, follow the ["cutting a release"](../README.md#cutting-a-release-for-maintainers--forks) guidelines to tag
and build new images for the binary.
