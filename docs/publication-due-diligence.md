# Publication due diligence — rv0.4.0

This records what was checked around the public rv0.4.0 publication. It is evidence, not a claim that the software has no defects.

## Completed publication checks

- The repository is public under Comfort Curators.
- The GitHub-visible `main` history was reviewed across the 97 commits reachable at publication time for high-signal secret patterns. No private-key blocks or obvious production GitHub, OpenAI, AWS, Google, or Slack credentials were found. The HMAC value committed for local examples is explicitly a test key.
- `.dev.vars` was checked around its introduction and was not tracked.
- The publication tree excludes `.env`, `.dev.vars`, backups/exports, private keys, build outputs and `node_modules`.
- Production credentials such as `CURATOM_HMAC_KEY`, `RAJ_TOKEN`, `TURNSTILE_SECRET` and `ZEPTO_TOKEN` are not assigned in source.
- Checked-in D1/R2/domain/owner identifiers are treated as public deployment identifiers, not authenticators.
- The repository contains the GNU AGPLv3 license text and third-party notices for the vendored Elixir dependency/build-tool surface.
- README, architecture, security, lineage, citation and Zenodo metadata distinguish shipped behavior from deferred work.
- Curatom rv0.2.0 / rv0.3.0 and Curatom Polyglot rv0.4.0 are described as a project lineage, not an incremental source upgrade.

## Verification still owned by the release checkout

Before creating the immutable Git tag, run `./scripts/release-check.sh` against the exact commit and run the real Cloudflare probes relevant to the final code diff. A local object-level Gitleaks/TruffleHog scan over all refs/objects is still worthwhile because a web-visible commit review cannot prove the absence of unreachable local objects or private refs.

Hosted CI is not used as release authority.

## Archival state

Curatom Polyglot rv0.4.0 is archived on Zenodo at DOI **10.5281/zenodo.22855407**. The GitHub release should point at the exact `rv0.4.0` tag used for the public source state.

The earlier `comfortcurators/Curatom` repository remains the historical implementation: rv0.2.0 is the earlier Zenodo artifact and rv0.3.0 is the later Google Cloud All Things Agentic hackathon submission/evaluation state. Polyglot is a from-scratch implementation in the same lineage.
