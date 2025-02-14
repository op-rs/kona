## Compression

OP Stack genesis files are typically around 9MB, which quickly bloats the binary size of applications that use the
`validation` package. As a result, genesis files are stored in this repository as compressed `.json.zst` files.

### Benchmarks

We investigated a number of different compression methods, including:

1. No compression
2. Gzipping each individual genesis file
3. Manually extracting each contract's bytecode into individual gzipped files, then referencing them in each
   genesis file by their hashes
4. Using zstd with a pre-trained dictionary

| Compression Method | Size   | Compression Ratio |
|--------------------|--------|-------------------|
| None               | 326M   | 0                 |
| gzip               | 5.39MB | 60:1              |
| Manual             | 2.1MB  | 155:1             |
| zstd               | 2.36MB | 138:1             |

While manual compression is most effective, it also requires custom serialization code. Therefore, we opted for zstd
to optimize for small size while still allowing the genesis files to be compressed and decompressed without changes to
their structure.

### Generating the Dictionary

The dictionary is stored in the `extra/dictionary` file. To generate it:

1. Use the `dump_genesis` tool to output full genesis JSON files in a directory.
2. Run `zstd --train <your-dir>/*.json`. This will output a `dictionary` file in the current directory.
3. Move the resulting dictionary to `extra/dictionary`.
