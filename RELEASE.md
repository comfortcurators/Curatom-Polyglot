# Release checklist

Curatom does not use hosted CI as release authority. The release authority is the exact commit plus evidence collected against that commit.

## Before the tag

1. Run `./scripts/release-check.sh` from the exact checkout you intend to publish. The script is strict: missing release tooling is a failure, not a skip.
2. Run an independent object-level Git-history secret scan over all refs. For example, use Gitleaks or TruffleHog on the local clone. The publication review already checked all 97 commits then reachable from the repository's only branch (`main`) for high-signal credential patterns, but a local object scanner is still the final belt-and-suspenders gate.
3. Run the real Cloudflare behavioral probes relevant to the final diff: owner routing, scratchpad routing, Valhalla read/snapshot/restore, handoff/outcome routing, and authentication paths.
4. Read [`docs/publication-due-diligence.md`](docs/publication-due-diligence.md), [`DEFERRED.md`](DEFERRED.md), and the release diff once more. Do not turn a deferred mechanism into a release claim.

For rv0.4.0, keep the lineage wording precise: `comfortcurators/Curatom` contains the earlier rv0.2.0 Zenodo artifact and the later rv0.3.0 Google Cloud All Things Agentic hackathon submission/evaluation state. `comfortcurators/Curatom-Polyglot` rv0.4.0 is a from-scratch implementation in the same project lineage, not an in-place source upgrade.

## Publish

Use the tag **`rv0.4.0`**. Tag the verified commit once. Make the repository public. Publish the GitHub release from that exact tag. Archive the same source state on Zenodo.

Zenodo rv0.4.0 is published in the existing Curatom version lineage at DOI `10.5281/zenodo.22855407`. Keep that DOI in `CITATION.cff`, `codemeta.json`, and the README. The source discontinuity from the earlier implementation is documented in `docs/lineage.md`.

Do **not** move or rewrite the published `rv0.4.0` tag after it is created.
