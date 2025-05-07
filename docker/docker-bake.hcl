variable "REGISTRY" {
  default = "ghcr.io"
}

variable "REPOSITORY" {
  default = "op-rs/kona"
}

variable "DEFAULT_TAG" {
  default = "kona:local"
}

variable "PLATFORMS" {
  // Only specify a single platform when `--load` ing into docker.
  // Multi-platform is supported when outputting to disk or pushing to a registry.
  // Multi-platform builds can be tested locally with:  --set="*.output=type=image,push=false"
  default = "linux/amd64,linux/arm64"
}

variable "GIT_REF_NAME" {
  default = "main"
}

variable "BIN_TARGET" {
  // The binary target to build in the generic target
  default = "kona-host"
}

variable "ASTERISC_TAG" {
  // The tag of `asterisc` to use in the `kona-asterisc-prestate` target.
  //
  // You can override this if you'd like to use a different tag to generate the prestate.
  // https://github.com/ethereum-optimism/asterisc/releases
  default = "v1.3.0"
}

variable "CANNON_TAG" {
  // The tag of `cannon` to use in the `kona-cannon-prestate` target.
  //
  // You can override this if you'd like to use a different tag to generate the prestate.
  // https://github.com/ethereum-optimism/optimism/releases
  default = "cannon/v1.5.0-alpha.1"
}

variable "CLIENT_BIN" {
  // The `kona-client` binary to use in the `kona-{asterisc/cannon}-prestate` targets.
  //
  // You can override this if you'd like to use a different `kona-client` binary to generate
  // the prestate.
  //
  // Valid options:
  // - `kona` (single-chain)
  // - `kona-int` (interop)
  default = "kona"
}

// Special target: https://github.com/docker/metadata-action#bake-definition
target "docker-metadata-action" {
  tags = ["${DEFAULT_TAG}"]
}

target "generic" {
  inherits = ["docker-metadata-action"]
  context = "."
  dockerfile = "docker/apps/kona_app_generic.dockerfile"
  args = {
    TAG = "${GIT_REF_NAME}"
    BIN_TARGET = "${BIN_TARGET}"
  }
  platforms = split(",", PLATFORMS)
}

target "asterisc-builder" {
  inherits = ["docker-metadata-action"]
  context = "docker/asterisc"
  dockerfile = "asterisc.dockerfile"
  platforms = split(",", PLATFORMS)
}

target "cannon-builder" {
  inherits = ["docker-metadata-action"]
  context = "docker/cannon"
  dockerfile = "cannon.dockerfile"
  platforms = split(",", PLATFORMS)
}

target "kona-asterisc-prestate" {
  inherits = ["docker-metadata-action"]
  context = "."
  dockerfile = "docker/fpvm-prestates/asterisc-repro.dockerfile"
  args = {
    CLIENT_BIN = "${CLIENT_BIN}"
    CLIENT_TAG = "${GIT_REF_NAME}"
    ASTERISC_TAG = "${ASTERISC_TAG}"
  }
  # Only build on linux/amd64 for reproducibility.
  platforms = ["linux/amd64"]
}

target "kona-cannon-prestate" {
  inherits = ["docker-metadata-action"]
  context = "."
  dockerfile = "docker/fpvm-prestates/cannon-repro.dockerfile"
  args = {
    CLIENT_BIN = "${CLIENT_BIN}"
    CLIENT_TAG = "${GIT_REF_NAME}"
    CANNON_TAG = "${CANNON_TAG}"
  }
  # Only build on linux/amd64 for reproducibility.
  platforms = ["linux/amd64"]
}
