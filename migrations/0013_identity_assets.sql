ALTER TABLE external_identities
    ADD COLUMN avatar_asset_sha256 text,
    ADD COLUMN avatar_asset_filename text,
    ADD COLUMN avatar_provenance jsonb NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN profile_refreshed_at timestamptz;

ALTER TABLE external_identities
    ADD CONSTRAINT external_identities_avatar_asset_check CHECK (
        (avatar_asset_sha256 IS NULL) = (avatar_asset_filename IS NULL)
        AND (
            avatar_asset_sha256 IS NULL
            OR (
                avatar_asset_sha256 ~ '^[0-9a-f]{64}$'
                AND avatar_asset_filename ~ '^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$'
                AND avatar_asset_filename !~ '\.\.'
            )
        )
    ),
    ADD CONSTRAINT external_identities_avatar_provenance_object_check CHECK (
        jsonb_typeof(avatar_provenance) = 'object'
    );

COMMENT ON COLUMN external_identities.avatar_url IS
    'Legacy provider-hosted avatar URL. Readable by API clients but not rendered by default.';
COMMENT ON COLUMN external_identities.avatar_asset_sha256 IS
    'Digest of an allowlisted same-origin identity asset mounted by the deployment.';
COMMENT ON COLUMN external_identities.profile_refreshed_at IS
    'When provider profile metadata was last refreshed successfully.';
