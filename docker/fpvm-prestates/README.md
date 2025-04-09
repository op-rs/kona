# `fpvm-prestates`

Images for creating reproducible prestate builds for various fault proof virtual machines.

## Usage

### `kona-client` + `asterisc` prestate artifacts

```sh
# Produce the prestate artifacts for `kona-client` running on `asterisc` (version specified by `asterisc_tag`)
just asterisc <kona_tag> <asterisc_tag>
```

### `kona-client` + `cannon` prestate artifacts

```sh
# Produce the prestate artifacts for `kona-client` running on `cannon` (version specified by `cannon_tag`)
just cannon <kona_tag> <cannon_tag>
```
