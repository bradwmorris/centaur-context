# Issue 131: Step 5 isolation and model follow-up

Historical checkpoint: **full integration remained unverified**, 2026-10-02.
The owner subsequently removed this certification gate; see the
[current scope and evidence](README.md). The operational finding below remains
unresolved and outside this documentation task. This follow-up retains the
passing component evidence in [the initial receipt](step5.md).

## Slack routing

The owner authorized small test messages in any Slack channel, subject to keeping
the working system intact. Read-only configuration inspection established that
the active bots use their existing capture and retrieval destinations. Other
app-specific secrets correspond to retired app configurations; the observed
callback configuration routes only the retained app, with a fallback 404.
No separate test callback to a disposable installation was established.

Changing a channel would not change those backend destinations. No test messages
were sent, existing callbacks were not redirected, and no competing event consumer
was started. The remaining input is a distinct test app/event route whose delivery
can reach the disposable installation without changing existing delivery. Sending
permission itself is no longer a blocker. Private app identities, channel details,
callback hostnames, and credentials are deliberately absent from this receipt.

## Context build provenance

The following command succeeded from clean branch commit
`69e8c2f357383e084b955d3cff9613a60a633b1a`:

```sh
docker build \
  --label org.opencontainers.image.revision=69e8c2f357383e084b955d3cff9613a60a633b1a \
  -t centaur-context:issue131-69e8c2f .
```

All application build layers were content-matched Docker cache hits. This was a
build against the checked-out source, not merely a retag of the old runtime image;
it was not an uncached compilation. Resulting manifest-list/image ID:
`sha256:df17d0c95eca95ffcbd85ecc19bf56074a052f6c6fb6ee3c25637c9d0081fc79`.
Platform manifest:
`sha256:e7923b2d633844236a2ff014a822861176a6b32cc218ac1d86aad87994aebb76`.
Config digest:
`sha256:aa28dc396f1634e7e3fe8806cf4924ef150f96a307c686690395e5e6bca4cc9d`.

The host then had about 33 GiB free disk and the existing Docker workload used
about 4.8 of 7.65 GiB memory. The build completed without resource reconfiguration
or cache pruning. Capacity is not a demonstrated blocker for this Context build.
A full fresh Centaur build and paired deployment remain untested.

## Supported broker attempt

A second disposable fixture ran that Context image and PostgreSQL on a private
Docker network, with loopback-only API bindings. Each container had a 256 MiB
memory limit and 0.5 CPU limit. Its database was
`centaur_context_test_131_model`, with new test-generated API/database credentials.
This fixture used a disposable database administrator role; it does not replace
the separate-application-role Kubernetes evidence from the first run.

The existing broker was enabled and configured for subscription access. Context
used its supported inference endpoint through a temporary loopback port-forward,
with the existing narrowly scoped Curator service bearer. The bearer was supplied
through a mode-0600 environment file, removed after container creation. No model
refresh token was copied, no second identity consumer was installed, and no shared
service configuration was changed. Maintenance and deterministic event capture
were disabled in the fixture.

Two fixture configuration omissions (the distinct legacy writer token and the
explicit Curator prompt version) were corrected before ingestion. These were
test-harness setup errors, not successful installation attempts. Afterward,
authenticated readiness passed and ingestion accepted a fictional human decision
about Project Harbor's private endpoints. The actual Context worker queued and
attempted extraction with `gpt-6-luna`, low effort.

The broker's configured proxy image could not be pulled. Its ephemeral proxy pod
entered `ImagePullBackOff`; the broker returned a validated `transport` failure
after 92,607 ms, with `retryable: true` and no provider status/code. Context
persisted that diagnostic. No model output or generated Memory was observed.
Similar broker startup failures were present before this fixture's request.
This finding is an existing service prerequisite, not evidence that the candidate
Centaur revision fails.

The Context worker began a second attempt before the observation loop reported a
terminal state. The disposable caller was stopped to prevent further retries;
its saved Run was still `running` with `attempts: 2`, not completed or successfully
reconciled. The [sanitized failure receipt](step5-model-failure.json) preserves
those facts. No model-extraction, later-Memory-retrieval, or billing success is
claimed. The shared broker was not repaired, restarted, or scaled.

## Remaining boundary

A future full-certification exercise would need a distinct Slack test route and
a working supported broker whose ephemeral sandbox images are available. Repair of the existing deployment
is outside this test's authorization; a separate working test broker is also a
valid way forward. Actual Slack capture/reply, successful Memory extraction,
later interaction retrieval, complete proxy/tool routing, the paired fresh
installation, and ordinary Centaur replies during a Context outage remain
unverified. No deliberate outage was introduced and no tested pair was published.

The disposable app/database containers, database volume, network, generated
credential files, and port-forward were removed. Shared deployment generations
and Ready replica counts matched their pre-test snapshot. No existing workload,
callback, database destination, CNI, or host allocation was changed.
