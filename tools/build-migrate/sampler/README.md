# Client-assisted sampler pins

`derive.py` finds every build-specific value that `crates/net/src/sampler.js`
reads by stable name: type definition indices, metadata usage cells, the Heavy
callback slot, the metadata globals, field offsets and types, instance sizes,
generic-context (RGCTX) slots and fingerprints of the transcribed methods. It
parses the public Build assets only. It never instantiates or runs target code.
[`../migrate.py`](../README.md) covers Net's other pins.

`ea1a68bd.json` holds the pins for the build the sampler targets.
`crates/net/tests/sampler.cjs` builds its invented heap from this file. It fails
any sampler read that does not land on a pinned field of the matching type, and
any pinned field, cell or RGCTX slot that the sampler never reads. For each new
build, save the public
`Build/*.wasm.gz` and `Build/*.data.gz` outside Git and run:

```sh
python3 -I tools/build-migrate/sampler/derive.py --build NEW_BUILD_ID WASM DATA \
  --expect tools/build-migrate/sampler/ea1a68bd.json --out NEW.json
```

`MOVED` lines are renumbered indices, cells, cell words, slots and metadata
globals; copy them into `sampler.js`. `CHANGED` lines are layout, field type,
RGCTX, type-kind, size, method-body or Unity changes, and the script exits 1.
Review each against source before moving the sampler. Replace the committed
JSON with the new build's file in the migration change.

The test lays out objects from these sections:

- `fields`: `[offset, type]` for each field that the sampler reads. Value-type
  offsets include the 8-byte object header.
- `enums`: the storage type of each enum stored in a pinned field.
- `bases`: the base classes whose fields a pinned type inherits.
- `generics`: the layouts of the generic instances that the sampler reads, by
  argument shape. Metadata stores no offsets for generic definitions, so
  `derive.py` computes them. It first checks that the same rules reproduce the
  metadata offsets of every pinned non-generic type.
- `runtime`: libil2cpp's `s_GlobalMetadata` and `s_GlobalMetadataHeader` cells
  and the type-definition triple that `type()` uses to decode type handles.
- `cellWords`: the usage word, `(kind << 29) | (index << 1) | 1`, that each
  cell holds until the client first runs a method that uses it. The sampler
  reads that word as "not used yet" and any other odd word as a wrong pin.

Metadata does not describe libil2cpp's own structures, such as `Il2CppClass` and
`MethodInfo`. The test's `RUNTIME` table holds their offsets for Unity
6000.6.5f1 and fails when the pins name another Unity version. Re-prove those
offsets from the new build's libil2cpp first.
[Sampler heap reads](../../../docs/research/sampler-heap-reads.md) records the
evidence.

Method fingerprints name managed callees and metadata cells but treat native and
shared generic callees as anonymous. Pair those callees by call position and
compare their bodies once per migration.
