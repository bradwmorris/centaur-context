-- Add curated intents without rewriting historical research Notes.
ALTER TABLE notes DROP CONSTRAINT notes_intent_check;
ALTER TABLE notes ADD CONSTRAINT notes_intent_check
    CHECK (intent IS NULL OR intent IN ('idea','excerpt','fact','insight','question'));
COMMENT ON COLUMN notes.intent IS
    'Current research intents: idea, excerpt, fact. insight and question remain for legacy Notes; NULL remains unclassified.';
