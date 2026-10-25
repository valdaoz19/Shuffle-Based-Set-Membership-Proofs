# Research Status

Status as of 2026-10-25. Research maintainer: `valda` (`valdaoz19`).

## Implemented Experiments

Phase 1 / E1 implements EP uncompressed, EP compressed and CF-H coordinate
openings over BLS12-381, each in plain and hidden mode. The index is public in
both modes. Plain discloses the coordinate value; hidden links it to a Pedersen
commitment without disclosing that value.

Canonical binary encodings, transcript domain separation and malformed-input
tests support implementation correctness checks. They are not an independent
security audit or a proof of the security of a new composition.

## Review and Campaign Status

EP compressed hidden uses a concrete shared-response composition. Its
multi-round extraction, padding/nullity, privacy and Fiat-Shamir arguments still
require review. Its proof adds 80 bytes relative to the plain construction;
approval of that size difference alone is not a security validation.
See [the complete construction and review obligations](docs/E1_COMPRESSED_HIDDEN.md).

The smoke profile is diagnostic: n=64, two warm-ups and five recorded attempts
per scheme/mode. The final six-cell full-grid campaign has not been completed.
The `full` command reports `blocked_pending_research_review`; do not remove this
gate or treat candidate timings as final academic results without resolving
the construction review.

Phase 2 permutation/partition and anonymous-index membership are not implemented
by the E1 coordinate-opening interface. The formal membership specification
contains target relations and abstract functionalities that must be distinguished
from implemented concrete protocols.

## Legacy Application Limits

The AML application remains a pragmatic demonstrator. Its explicit mode uses
sigma rerandomization, implicit uses a Merkle path, and hybrid uses bucketed
subset plus sigma membership. It does not implement full Bayer-Groth, a
production VC membership protocol or a global partition proof.

Three HTTP services provide genuine process/network boundaries. However,
shuffle and threshold response checks still rely on metadata/flags rather than
independent verification of the complete corresponding transcripts. PayPay
state/history/todo are dummy and in memory. Its `paypay-proof-{request_id}` audit
handle is not the cryptographic hash used by the separate all-in-one SQLite audit.
PayPay creates demo transaction commitments; Wallet obtains their handles during
setup. Refer to the code and [demo guide](PANDUAN_DEMO.md) when describing the flow.

## Reproducibility

The compiler baseline is Rust 1.96.0; dependency resolution is pinned by
`Cargo.lock`. CI also checks stable compatibility. Compile inside this checkout
and run the experiment from its root after committing changes so that Git and
build provenance identify the intended source. Do not reuse a binary built in
another source directory for a new provenance record.

The initial hardware methodology targets the available Mac. Some environment
collection commands are macOS-specific; review OS/hardware metadata collection
before claiming a comparable Linux benchmark campaign. Passing Linux unit tests
does not establish performance equivalence across machines.

Commit generated research data separately only through an explicit dataset
release process. Until then, outputs remain local, and any shared report should
include raw attempts, configuration, schedule, environment, binary/source hashes
and the exact analysis version. Existing diagnostic records retain their
original configuration and date.
