# Client-assisted sampler pins

`derive.py` finds every build-specific value in `crates/net/src/sampler.js` and
`crates/net/tests/sampler.cjs` by stable name: type definition indices, metadata
usage cells, the typedef bound, the Heavy callback slot, field offsets,
generic-context (RGCTX) slots and fingerprints of the transcribed methods. It
parses the public Build assets only. It never instantiates or runs target code.
[`../migrate.py`](../README.md) covers Net's other pins.

`ea1a68bd.json` holds the pins for the build the sampler targets. For each new
build, save the public `Build/*.wasm.gz` and `Build/*.data.gz` outside Git and
run:

```sh
python3 -I tools/build-migrate/sampler/derive.py --build NEW_BUILD_ID WASM DATA \
  --expect tools/build-migrate/sampler/ea1a68bd.json --out NEW.json
```

`MOVED` lines are renumbered indices, cells and slots; copy them into both files.
`CHANGED` lines are layout, RGCTX, type-kind, size, method-body or Unity changes,
and the script exits 1. Review each against source before moving the sampler.
Replace the committed JSON with the new build's file in the migration change.

Method fingerprints name managed callees and metadata cells but treat native and
shared generic callees as anonymous. Pair those callees by call position and
compare their bodies once per migration.
