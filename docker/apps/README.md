# `docker-apps`

This directory contains a generic dockerfile that accepts two arguments: `TAG` and `BIN_TARGET`. It clones the `kona`
repository at the specified `TAG`, and builds the `BIN_TARGET` with the `release` profile, before finally copying it
into an `ubuntu:22.04` image with the binary in the PATH and set as the image's entrypoint.

The `generic` target in the [docker-bake.hcl](../docker-bake.hcl) supports publishing any binary in the kona repository
by specifying the binary name as the target. Optionally, the target can be overridden in the [docker-bake.hcl](../docker-bake.hcl) like so.

```hcl
target "<target-name>" {
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

The [docker release workflow](../../.github/workflows/docker.yaml) will **first** check if a target is available,
using that target for the docker build. If the workflow can't find the target, it will fallback to the "generic"
target specified in the [docker-bake.hcl](../docker-bake.hcl).

To cut a release for a generic binary, or an overridden target for that matter, follow the guidelines specified
in the ["cutting a release"](../README.md#cutting-a-release-for-maintainers--forks) section. This workflow allows
you to trigger a release just by pushing a tag to kona automagically, for any binary. No code changes needed :)
