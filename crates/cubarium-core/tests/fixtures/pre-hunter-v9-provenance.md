# Genuine schema9 continuation before hunter implementation

These fixtures were generated BEFORE the hunter worker began editing core, using
core `b47eacc` and the release host at root checkpoint `1d7b386`. Concurrent art
work was present but unused: both runs used `--sink none`, no art, no HTTP/shim.
The binary was frozen at `/tmp/cubarium-pre-hunter-v9-fN4Cls/cubarium-v9`, SHA256
`725305bc6bf8be924e5e46d5663b34bd6396855861af2933f671fb4ced421d0f`.

Source: committed genuine `live-v8-172800.cubw`, copied into that NEW temporary
state directory. It was resumed for600ticks to create schema9 at173400, then
resumed again for600ticks to174000. Exact command both times:

```text
cubarium-v9 run --sink none --state /tmp/cubarium-pre-hunter-v9-fN4Cls
  --speed 0 --seconds 30 --require-resume
```

The live directory/process were never used as destinations. The first snapshot
already contains600ticks of signed accounting corrections; this is not a fixture
with freshly zeroed extension bytes. Both retain actual admitted care history.
The second run validated the first on decode; root also checked both headers and
payload CRC32 independently from raw file bytes.

| File | Tick / population | Full schema9 hash (hex) | SHA256 |
| --- | --- | --- | --- |
| pre-hunter-v9-173400.cubw | 173400 /80 | 134f4db0135d8a0a | bd3b4e2ead0298806f49375e82b1c56335b1028d4cfbee7dc7b386a7d24a12a8 |
| pre-hunter-v9-173400-plus600.cubw | 174000 /81 | 854dce1766d905d3 | 586543dc7254ea19a28fff88d9c169bc35d72c2aa8489fb06bef34359aacc3c4 |

The empty-hunter-extension schema10 continuation must preserve the entire
schema9 payload, including care and existing signed corrections, not merely its
population or the older ecology projection hash. Header build IDs need not be
equal when a new binary re-encodes the payload.
