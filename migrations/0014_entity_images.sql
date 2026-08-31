ALTER TABLE entities
    ADD COLUMN image_url text;

ALTER TABLE entities
    ADD CONSTRAINT entities_image_url_check CHECK (
        image_url IS NULL
        OR (
            char_length(image_url) BETWEEN 1 AND 2048
            AND image_url ~ '^https://'
        )
    );

COMMENT ON COLUMN entities.image_url IS
    'Optional human-curated HTTPS avatar or logo reference. Image bytes are not fetched or stored by Centaur Context.';
