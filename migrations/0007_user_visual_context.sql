ALTER TABLE external_identities
    ADD COLUMN avatar_url text;

ALTER TABLE external_identities
    ADD CONSTRAINT external_identities_avatar_url_check CHECK (
        avatar_url IS NULL
        OR (
            char_length(avatar_url) BETWEEN 1 AND 2048
            AND avatar_url ~ '^https?://'
        )
    );
