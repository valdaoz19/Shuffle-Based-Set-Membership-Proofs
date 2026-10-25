# Compressed EP hidden: concrete construction for review

**Research status: construction_review_required.** This is an executable candidate,
not a simulation and not a claim that Protocol 5 directly handles a group-valued
second equation. Full E1 results remain gated until this composition is reviewed.

Source: [Attema-Cramer, Protocol 5, Section 5 and Appendix E](https://ir.cwi.nl/pub/30160/30160.pdf).
The code uses the paper's recursive linear-form compression and a shared-response
outer Sigma protocol. It does **not** pretend that writing `CompressedHOpen.Prove`
constitutes a protocol, and it does not import a generic Dock homomorphism proof
from a different paper under the Attema-Cramer label.

## Relation and parameters

Work additively over prime-order BLS12-381 G1 and Fr. Independent hash-derived bases:
B_0,...,B_n, padding bases, K, G,H. No known discrete-log relations.

```text
Public:  R, k, C; 0 <= k < n
Witness: v=(v_0,...,v_{n-1}), s, r
R = sum_{i=0}^{n-1} v_i B_i + s B_n
C = v_k G + r H
```

Let d=next_power_of_two(n+1), ell=log2(d), and
w=(v_0,...,v_{n-1},s,0,...,0) in Fr^d.
Padding positions are not alternative message coordinates. The blinding position
is n, never an allowed opened index.

All transcript challenges bind scheme/mode, parameter digest, full statement and
earlier messages. For unused padding coordinates derive independent alpha_j from
the statement-bound transcript and define

```text
L(w) = w_k + sum_{j=n+1}^{d-1} alpha_j w_j.
```

Honest padding is zero, so L(w)=v_k. This is an explicit random linear nullity
check, not a silent assumption that a malicious prover must use the API's zero
padding. Rewinding/binding arguments for the combined nullity check are one of
the items requiring review below.

## Outer shared-response Sigma

Sample a_i for i<=n and eta independently uniformly in Fr; a_padding=0.

```text
Prover first messages:
  A_R = sum_i a_i B_i
  A_C = a_k G + eta H
  append A_R, A_C
  c0 = FS(transcript)

Prover responses sent before compression:
  z   = a_k + c0 v_k
  phi = eta + c0 r
  append z, phi

Verifier checks:
  z G + phi H = A_C + c0 C

Underlying vector response (NOT sent):
  u = a + c0 w
  sum_i u_i B_i = A_R + c0 R
  L(u) = z
```

Only z is revealed, not v_k. It is masked by a_k. Critically, that SAME z is both
the Pedersen equation response and the linear output required by compression.

## Linear-form compression

Derive c1 after z,phi. Set F=c1*L and

```text
Q = A_R + c0 R + K (c1 z)
Q = sum_i u_i B_i + K F(u).
```

Use Protocol 5's recursive reduction, with current vectors u,B,F:

```text
While length(u)>2:
  split u=(u_L,u_R), B=(B_L,B_R), F=(F_L,F_R)
  A = <B_R,u_L> + K F_R(u_L)
  Bcross = <B_L,u_R> + K F_L(u_R)
  send A,Bcross; append them to transcript
  c = FS(transcript)
  Q <- A + c Q + c^2 Bcross
  B <- c B_L + B_R
  F <- c F_L + F_R
  u <- u_L + c u_R

Send u_0,u_1.
Accept iff Q = u_0 B_0 + u_1 B_1 + K(F_0 u_0 + F_1 u_1),
AND the outer Pedersen equation accepted.
```

The implementation has a recursive reference verifier and an original-basis
weighted-MSM verifier. Tests compare them. The latter avoids recursively folding
every generator in verification; it still processes all d original bases.

## Correctness and security obligations

Correctness of each fold follows by expansion:

```text
<c B_L+B_R, u_L+c u_R> + K(c F_L+F_R)(u_L+c u_R)
 = A + c Q + c^2 Bcross.
```

The outer shared response satisfies the Pedersen equation directly. Given two
accepting outer transcripts with fixed A_R,A_C and c0 != c0', and extracted inner
responses u,u', z,z', phi,phi', the proposed extraction is:

```text
w* = (u-u')/(c0-c0')
x* = (z-z')/(c0-c0')
r* = (phi-phi')/(c0-c0')
R = <B,w*>; L(w*)=x*; C=x*G+r*H.
```

This is not yet enough by itself: the argument must additionally establish zero
padding, binding to one opening across the initial nullity challenges, and valid
multi-round extraction of the inner reduction. For privacy, the outer full vector
response is uniformly masked on real coordinates; one must prove that the folded
messages preserve the corresponding SHVZK property, including the correlated z
and the fixed-zero padding. The Fiat-Shamir composition requires a random-oracle
argument consistent with those round dependencies. Unit tests cannot establish
these statements. No complete formal extractor or reduction theorem is claimed.

## Actual serialized proof

```text
A_R (48), A_C (48), z (32), phi (32),
ell-1 pairs of fold points (96 per pair), u_0 (32), u_1 (32).

hidden_bytes = 160 + 96(ell-1) + 64 = 96 ell + 128
plain_bytes  =  80 + 96(ell-1) + 64 = 96 ell + 48
```

Plain Protocol 5 sends t=L(a), A_R instead of A_R,A_C,z,phi.
For the full grid n is a power of two, so ell=ceil(log2(n+1)).

| n | Plain actual B | Hidden candidate actual B | Rev7 hidden approximate B |
| ---: | ---: | ---: | ---: |
| 64 | 720 | 800 | 720 |
| 1024 | 1104 | 1184 | 1104 |
| 4096 | 1296 | 1376 | 1296 |

C adds 48 transmitted bytes, separately. The additional 80 bytes are explained
by replacing the plain first message (one point, one scalar) with the hidden
outer messages (two points, two scalars). The report's approximate formula assumed
the hidden relation could be encoded as one linear-form opening. We have NOT
silently made that assumption or forced output sizes to match it.

## Decision requested from Mas Valda

Review this concrete shared-response compression, including padding nullity and
multi-round extraction/privacy. Decide whether to accept this explicit variant
with measured size/work, or require a construction following Appendix E's shell
and multiple-commitment nullity machinery more literally. The second choice is
a protocol change, not just a serializer optimization.

Until resolved, EP uncompressed and CF-H remain executable, all six cells may be
smoke-tested as diagnostics, and no final six-cell campaign is claimed complete.
