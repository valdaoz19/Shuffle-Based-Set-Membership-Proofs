# Set Membership Proof Formal Protocol Specification

This document specifies three formal Set Membership Proof protocols for the
ZKP AML three-entity model: User Wallet owner, Alipay+ as verifier, and PayPay
as prover. The document is written as mathematical protocol pseudocode.

## 1. Commitment Layer and Set Semantics

```text
Public parameter:
  pp = (G, g_id, h)

Primary Pedersen ID commitment:
  Com_pp(id; r) = g_id*id + h*r

Transaction-ID commitment:
  c_tid = Com_pp(tid; r_tid)

Rerandomization:
  ReRand_pp(C; rho) = C + h*rho

Object domain:
  X = (x_0, ..., x_{n-1})
  x_i is the logical object or transaction identifier record.

Commitment representation:
  S = (C_0, ..., C_{n-1})
  C_i = Com_pp(id_i; r_i)

Set namespace:
  set_id = H(provider_id || policy_id || epoch || root || version)

Optional AML value-binding extension:
  IBC_pp(id, v; r) = g_id*id + g_val*v + h*r
```

For an ID-only set membership statement, the primary formal commitment is a
Pedersen ID commitment. IBC is not required when the statement has no value
component. IBC is kept only as an AML extension for a later statement that must
bind `id`, amount/value, and policy context in one commitment.

`X` is the object set. `S` is the committed representation of `X`. The verifier
checks membership over commitments because the payment flow should not reveal
the raw transaction or identity object. The implicit and hybrid protocols are
still useful over committed sets because they reduce communication,
verification cost, or subset size while preserving the hidden object layer.

`set_id` binds every proof to one public set namespace. It prevents accidental
or malicious verification against the wrong root, epoch, policy, provider, or
version.

```text
Membership statement:
  stmt = (pp, S, c_tid, set_id)

General relation:
  R_member(stmt, w) = 1 iff
    exists i in {0,...,n-1}, rho in F:
      c_tid = ReRand_pp(S[i]; rho)
```

The target commitment is not required to equal `S[i]` byte-for-byte. It is a
valid member when it is rerandomization-equivalent to one committed element of
`S`.

## 2. Function Semantics for Implicit and Hybrid Protocols

This section expands the function names used by Protocol 2 and Protocol 3.
The important point is not only that a function exists, but which relation it
proves.

| Function | Public input | Witness | Relation proven | Privacy note |
| --- | --- | --- | --- | --- |
| `VC.Commit(V)` | Ordered vector `V=(V_0,...,V_{n-1})` | None | Outputs root `R_V` that binds each index and value. | No membership privacy by itself. |
| `VC.Open(V,i)` | `V`, index `i` | Vector auxiliary data | Outputs `aux_i` for element `V_i`. | Public opening reveals `i`. |
| `VC.Verify(R_V,i,V_i,aux_i)` | `R_V`, `i`, `V_i`, `aux_i` | None | Accepts iff `V_i` is the value committed at index `i` under `R_V`. | This is verifiable opening, not private set membership. |
| `PermVC.ProvePerm(pp,S,R_V; sigma,Delta,aux_all)` | `pp`, `S`, `R_V` | `sigma`, `Delta`, openings for `V` | Proves `R_V=VC.Commit(V)`, `V_k=ReRand_pp(S[sigma(k)];delta_k)` for all `k`, and `sigma` is a permutation. | Hides the permutation and rerandomization witnesses. |
| `PermVC.VerifyPerm(pp,S,R_V,pi_perm_vc)` | `pp`, `S`, `R_V`, `pi_perm_vc` | None | Accepts iff `R_V` commits to a permutation/rerandomization of `S`. | Prevents PayPay from committing to unrelated vector `V`. |
| `ZKVCMember.Prove(pp,R_V,c_tid; i,aux_i,rho)` | `pp`, `R_V`, `c_tid` | hidden `i`, `aux_i`, `rho` | Proves `VC.Verify(R_V,i,V_i,aux_i)=1` and `c_tid=ReRand_pp(V_i;rho)`. | Keeps `i` and `V_i` hidden. |
| `CommitBucket(B_j)` | ordered bucket vector `B_j` | None | Outputs bucket root `D_j` that binds all elements and positions in `B_j`. | Bucket privacy depends on later proof shape. |
| `CommitGlobal(D_0,...,D_{m-1})` | bucket roots `D_0,...,D_{m-1}` | None | Outputs global root `R_S` binding every bucket root. | Does not reveal bucket content. |
| `ProvePartition(pp,S,R_S; B_0,...,B_{m-1})` | `pp`, `S`, `R_S` | buckets and assignment/permutation witnesses | Proves every bucket element is a rerandomization of an element in `S`, all elements of `S` appear exactly once, and `R_S=CommitGlobal(CommitBucket(B_0),...,CommitBucket(B_{m-1}))`. | Hides assignment details when implemented as ZK proof. |
| `VerifyPartition(pp,S,R_S,pi_partition)` | `pp`, `S`, `R_S`, `pi_partition` | None | Verifies global bucket consistency with `S`. | Prevents arbitrary bucket construction. |
| `ProveGlobal/VerifyGlobal` | `R_S`, bucket claim/proof | hidden or public bucket witness | Proves the used bucket commitment belongs under `R_S`. | Bucket index `j` can be hidden when bucket privacy is required. |
| `BucketMember.Prove/Verify` | bucket/global root and `c_tid` | hidden bucket member position and rerandomization | Proves `c_tid` is a rerandomization-equivalent member of the committed bucket. | Avoids revealing the full bucket; can hide position and bucket index. |

## 3. Protocol 1: Explicit Shuffle-Based Set Membership

```text
Protocol 1: Explicit Shuffle-Based Set Membership

Parameter:
  pp = (G, g_id, h)
  S = (C_0, ..., C_{n-1})
  Shuffle = (Setup, Prove, Verify)
  ZKLinkAny = (Prove, Verify)

Input:
  Prover PayPay:
    stmt = (pp, S, c_tid, set_id)
    witness w_exp = (i, sigma, Delta, rho, tau)
    i in {0,...,n-1}
    tau in {0,...,n-1}
    sigma is a permutation of {0,...,n-1}
    sigma(tau) = i
    c_tid = ReRand_pp(S[i]; rho)
    Delta = (delta_0, ..., delta_{n-1})

  Verifier Alipay+:
    stmt = (pp, S, c_tid, set_id)
    proof pi_exp = (C_prime, pi_shuffle, pi_member)

Output:
  ACCEPT if c_tid is proven rerandomization-equivalent to one element of S.
  ABORT if shuffle verification or anonymous member-link verification fails.

Setup:
  1. Verifier fixes set_id for the active root/epoch/policy.
  2. Prover and verifier agree on pp and the committed set S for set_id.

Prover:
  3. Choose hidden output position tau and permutation sigma with sigma(tau)=i.
  4. For k = 0,...,n-1:
       C_prime[k] = ReRand_pp(S[sigma(k)]; delta_k)
  5. Compute:
       pi_shuffle = Shuffle.Prove(pp, S, C_prime; sigma, Delta)
  6. Let:
       rho_prime = rho - delta_tau
       c_tid = ReRand_pp(C_prime[tau]; rho_prime)
  7. Compute anonymous member-link proof:
       pi_member = ZKLinkAny.Prove(pp, C_prime, c_tid; tau, rho_prime)
  8. Send:
       pi_exp = (set_id, C_prime, pi_shuffle, pi_member)

Verifier:
  9. Check set_id is the expected namespace.
 10. Check |C_prime| = |S|.
 11. Verify shuffle:
       b_shuffle = Shuffle.Verify(pp, S, C_prime, pi_shuffle)
 12. Verify hidden link to some shuffled output:
       b_member = ZKLinkAny.Verify(pp, C_prime, c_tid, pi_member)
 13. ACCEPT iff b_shuffle = 1 and b_member = 1.
     Otherwise ABORT.

Accept/Abort Condition:
  ACCEPT iff:
    set_id is valid
    AND |C_prime| = |S|
    AND Shuffle.Verify(pp, S, C_prime, pi_shuffle) = 1
    AND ZKLinkAny.Verify(pp, C_prime, c_tid, pi_member) = 1

  ABORT otherwise.

Relation:
  R_exp(stmt, w_exp) = 1 iff:
    exists i, tau, sigma, Delta, rho such that:
      sigma is a permutation
      sigma(tau) = i
      for all k:
        C_prime[k] = ReRand_pp(S[sigma(k)]; delta_k)
      c_tid = ReRand_pp(S[i]; rho)
      equivalently:
        exists rho_prime:
          c_tid = ReRand_pp(C_prime[tau]; rho_prime)

Security Intuition:
  The shuffle proof shows that C_prime is a permutation/rerandomization of S.
  The anonymous link proof shows that c_tid links to one hidden element of
  C_prime. The verifier does not learn i, tau, or the target mapping.
```

## 4. Protocol 2: Implicit Vector Commitment Set Membership

```text
Protocol 2: Implicit Vector Commitment Set Membership

Parameter:
  pp
  S = (C_0, ..., C_{n-1})
  VC = (Setup, Commit, Open, Verify)
  PermVC = (ProvePerm, VerifyPerm)
  ZKVCMember = (Prove, Verify)

Input:
  Prover PayPay:
    stmt = (pp, S, c_tid, set_id)
    witness w_vc = (i, sigma, Delta, rho, aux_i)
    V[i] = ReRand_pp(S[sigma(i)]; delta_i)
    c_tid = ReRand_pp(V[i]; rho)

  Verifier Alipay+:
    pp, set_id
    public committed set descriptor for S
    vector root R_V
    proof pi_perm_vc
    target c_tid
    proof pi_zk_vc

Output:
  ACCEPT if R_V commits to a permutation/rerandomization of S and c_tid is
  proven to open at one hidden index of R_V.
  ABORT if permutation consistency or hidden membership verification fails.

Setup:
  1. Prover chooses permutation sigma and rerandomization vector Delta.
  2. For k = 0,...,n-1:
       V[k] = ReRand_pp(S[sigma(k)]; delta_k)
  3. Compute vector commitment:
       R_V = VC.Commit(V)
  4. Compute vector permutation consistency proof:
       pi_perm_vc = PermVC.ProvePerm(pp, S, R_V; sigma, Delta, aux_all)
  5. Register:
       set_id = H(provider_id || policy_id || epoch || R_V || version)

Prover:
  6. Find hidden index i such that:
       c_tid = ReRand_pp(V[i]; rho)
  7. Compute hidden-index VC membership proof:
       pi_zk_vc = ZKVCMember.Prove(pp, R_V, c_tid; i, aux_i, rho)
  8. Send:
       (set_id, R_V, pi_perm_vc, c_tid, pi_zk_vc)

Verifier:
  9. Check set_id binds to the expected R_V and epoch.
 10. Verify vector-root consistency with S:
       b_perm = PermVC.VerifyPerm(pp, S, R_V, pi_perm_vc)
 11. Verify hidden-index membership:
       b_member = ZKVCMember.Verify(pp, R_V, c_tid, pi_zk_vc)
 12. ACCEPT iff b_perm = 1 and b_member = 1.
     Otherwise ABORT.

Accept/Abort Condition:
  ACCEPT iff:
    set_id is valid for R_V
    AND PermVC.VerifyPerm(pp, S, R_V, pi_perm_vc) = 1
    AND ZKVCMember.Verify(pp, R_V, c_tid, pi_zk_vc) = 1

  ABORT otherwise.

Relation:
  R_imp(stmt, w_vc) = 1 iff:
    exists V, i, sigma, Delta, rho, aux_i such that:
      R_V = VC.Commit(V)
      V is a permutation/rerandomization of S:
        for all k:
          V[k] = ReRand_pp(S[sigma(k)]; delta_k)
      c_tid = ReRand_pp(V[i]; rho)
      PermVC.VerifyPerm(pp, S, R_V, pi_perm_vc) = 1
      ZKVCMember.Verify(pp, R_V, c_tid, pi_zk_vc) = 1

Security Intuition:
  A plain public opening (i, value, pi_vc) only proves verifiable opening and
  reveals i. The protocol therefore uses a zero-knowledge VC membership proof
  so that the verifier learns membership under R_V, while i and the opened
  vector element remain hidden. The permutation proof prevents PayPay from
  committing to an unrelated vector.
```

## 5. Protocol 3: Hybrid Global Commitment Set Membership

```text
Protocol 3: Hybrid Global Commitment Set Membership

Parameter:
  pp
  S = (C_0, ..., C_{n-1})
  m = number of buckets
  Bucket = (Assign, CommitBucket)
  Global = (CommitGlobal, VerifyGlobal)
  Partition = (ProvePartition, VerifyPartition)
  BucketMember = (Prove, Verify)

Input:
  Prover PayPay:
    stmt = (pp, S, c_tid, set_id)
    witness w_hyb = (j, S_j, sigma_j, Delta_j, rho, w_global, w_member)
    c_tid is rerandomization-equivalent to one hidden element of S_j

  Verifier Alipay+:
    pp, set_id
    public committed set descriptor for S
    global root R_S
    partition proof pi_partition
    hidden-bucket proof pi_global
    bucket membership proof pi_bucket_member

Output:
  ACCEPT if the committed bucket system is globally consistent with S and
  c_tid is proven to belong to one hidden committed bucket.
  ABORT if partition, global, or bucket membership verification fails.

Global Setup:
  1. Prover partitions and optionally shuffles/rerandomizes S into buckets:
       B_0, ..., B_{m-1}
  2. For each bucket j:
       D_j = CommitBucket(B_j)
  3. Compute:
       R_S = CommitGlobal(D_0, ..., D_{m-1})
  4. Compute partition consistency proof:
       pi_partition = ProvePartition(pp, S, R_S; B_0,...,B_{m-1})
  5. Register:
       set_id = H(provider_id || policy_id || epoch || R_S || version)

Prover:
  6. Choose hidden bucket j and hidden member position tau such that:
       exists rho:
         c_tid = ReRand_pp(B_j[tau]; rho)
  7. Compute hidden bucket-global proof:
       pi_global = ProveGlobal(pp, R_S, D_j; j, w_global)
  8. Compute hidden bucket membership proof:
       pi_bucket_member =
         BucketMember.Prove(pp, D_j, c_tid; tau, rho, w_member)
  9. Send:
       (set_id, R_S, pi_partition, c_tid, pi_global, pi_bucket_member)

Verifier:
 10. Check set_id binds to expected R_S and epoch.
 11. Verify global partition consistency:
       b_part = VerifyPartition(pp, S, R_S, pi_partition)
 12. Verify that some committed bucket is under R_S:
       b_global = VerifyGlobal(pp, R_S, pi_global)
 13. Verify membership inside that hidden bucket:
       b_member = BucketMember.Verify(pp, R_S, c_tid, pi_bucket_member)
 14. ACCEPT iff b_part = 1, b_global = 1, and b_member = 1.
     Otherwise ABORT.

Accept/Abort Condition:
  ACCEPT iff:
    set_id is valid for R_S
    AND VerifyPartition(pp, S, R_S, pi_partition) = 1
    AND VerifyGlobal(pp, R_S, pi_global) = 1
    AND BucketMember.Verify(
          pp, R_S, c_tid, pi_bucket_member
        ) = 1

  ABORT otherwise.

Relation:
  R_hyb(stmt, w_hyb) = 1 iff:
    exists buckets B_0,...,B_{m-1}, hidden j, tau, rho such that:
      R_S = CommitGlobal(D_0, ..., D_{m-1})
      for all j:
        D_j = CommitBucket(B_j)
      VerifyPartition(pp, S, R_S, pi_partition) = 1
      VerifyGlobal(pp, R_S, pi_global) = 1
      c_tid = ReRand_pp(B_j[tau]; rho)
      BucketMember.Verify(
        pp, R_S, c_tid, pi_bucket_member
      ) = 1

  Mathematical implication:
    VerifyPartition => union(B_j) is a permutation/rerandomization of S
    VerifyGlobal    => the hidden bucket commitment belongs under R_S
    BucketMember    => c_tid belongs to that hidden bucket
    Therefore c_tid belongs to S.

Security Intuition:
  The partition proof prevents arbitrary bucket construction. The global proof
  prevents an arbitrary bucket from being used outside R_S. The bucket member
  proof avoids revealing the full set and can be designed to hide the bucket
  index j when bucket privacy is required.
```

## 6. Stakeholder Feedback Resolution Matrix

| No. | Feedback | Formal revision |
| --- | --- | --- |
| 1 | Why IBC instead of a standard commitment? | The primary formal commitment is now Pedersen ID commitment. IBC is documented only as an optional AML extension for value/policy binding. |
| 2 | What is `set_id`? | `set_id` is defined as `H(provider_id || policy_id || epoch || root || version)`, a namespace binding proof verification to one root and policy epoch. |
| 3 | Is `S` a set of objects or commitments? | `X` is the object set. `S` is the commitment representation. Implicit and hybrid are motivated by cheaper verification and smaller disclosure over committed sets. |
| 4 | `c_tid` is not equal to `S[i]`; it is `Rerand(S[i])`. | The general relation is changed to `exists i,rho: c_tid = ReRand_pp(S[i]; rho)`. |
| 5 | `sigma(0)=i` should not be proved because verifier must not know `i`. | Explicit protocol now uses hidden `tau` with `sigma(tau)=i`; verifier checks `ZKLinkAny` without learning `i` or `tau`. |
| 6 | Public VC opening reveals `i` and is not privacy-preserving set membership. | The new function semantics define `VC.Open/Verify` as public opening only, then define `PermVC` and `ZKVCMember` relations for hidden-index membership. |
| 7 | Hybrid needs global consistency and bucket privacy details. | The new function semantics define `CommitBucket`, `CommitGlobal`, `ProvePartition`, `VerifyGlobal`, and hidden bucket membership. |
