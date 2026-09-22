# Sources overlay views and bounded preview images

Issue: #92. Execution started at the owner's request. Tasks Kanban is unchanged.

## Design review

Sources gets its own typed collection adapter, with the existing host query,
sort, pagination and detail navigation. Task API 1 remains compatible. Manifest
schema/API 1 gains `sources` as an explicitly supported section; exact core pins
still reject older hosts. Separate overlay Grid and Board components own layout.
The proposed Board groups by existing source kind, without writes or new state.
Overlay adoption target and final grouping await the owner's response.

## Network/security review and selected boundary

A generic, optional human-listener endpoint accepts only a canonical Source UUID,
never an arbitrary fetch URL. `SOURCE_THUMBNAILS_ENABLED=true` enables it; stock
installations make no outbound image requests. Images are fetched server-side and
served at the same origin. No new listener or public ingress is introduced. The
existing human-listener network trust boundary remains essential: this is not
agent authentication or a per-user authorization boundary.

Only HTTPS on port 443 is accepted, without URL credentials. Every fetch and
redirect validates the destination. DNS results must consist entirely of public
IPv4 addresses; IPv6-only hosts are deliberately unsupported in v1. Connections
pin validated DNS addresses, retain TLS hostname verification, disable proxies,
and never forward browser headers/cookies/credentials. Loopback, private,
link-local, CGNAT, reserved, documentation, multicast and transition addresses
are rejected. Redirects are handled manually with a strict cap. Metadata-derived
image URLs undergo the same validation; HTML is parsed as data, never executed.

YouTube watch/short/embed URLs resolve to its public thumbnail host. Generic
articles use publisher image metadata; unsupported/missing images fall back.
Only bounded JPEG/PNG/WebP raster images are served, with signature and dimension
checks, fixed response MIME, nosniff and no-referrer. SVG/HTML payloads are never
served. Requests have bounded time, body size and concurrency. Preview state is
an in-memory bounded cache keyed by canonical URL, not Source/Artifact data;
refreshes cannot alter Source revisions or Events. Success and negative results
have explicit TTLs. Forced refresh is throttled. Cache eviction/restart may
require refetching. Publisher availability is not guaranteed.

Privacy: the publisher/image host sees the Context server IP and requested URL,
not browser cookies or referrers. Operators must opt in knowingly. URLs may carry
sensitive query parameters already present in canonical Sources; no query values
are logged, but operators should not enable this for secret-bearing URLs.

## Review findings and acceptance boundary

The above design avoids an arbitrary-URL proxy and same-origin active-content
hosting. Residual risks are publisher tracking of the server, upstream changes,
public-host bandwidth consumption and the existing trusted-human listener.
Bounded cache/concurrency, opt-in operation and fallbacks mitigate these risks.
Tests must cover URL/IP/redirect restrictions, malformed image metadata, limits,
cache freshness, source URL changes and disabled operation before enabling the
isolated live smoke test. Production enablement remains a separate owner review.

Heavy compilation and container builds run remotely. Use a dedicated
`centaur_context_test_*` database and resource-limited local demo containers.
