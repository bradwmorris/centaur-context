-- Enforce the forward-looking Object description limit without scanning or
-- rewriting legacy rows. Issue #52 owns remediation and final validation.
ALTER TABLE objects
    ADD CONSTRAINT objects_description_600_characters_check
    CHECK (char_length(btrim(description)) <= 600) NOT VALID;
