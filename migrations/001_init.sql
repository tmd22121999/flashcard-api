-- migrations/001_init.sql
-- Run with: sqlx migrate run

-- ─────────────────────────────────────────────
-- USERS
-- ─────────────────────────────────────────────
CREATE TABLE users (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username    VARCHAR(64)  NOT NULL UNIQUE,
    email       VARCHAR(255) NOT NULL UNIQUE,
    password_hash TEXT       NOT NULL,
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- ─────────────────────────────────────────────
-- FLASHCARDS  (base / canonical data)
-- Only admin / system inserts canonical cards.
-- Regular users can override fields via user_card_overrides.
-- ─────────────────────────────────────────────
CREATE TABLE flashcards (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    word        VARCHAR(255) NOT NULL,
    front_text  TEXT         NOT NULL,
    back_text   TEXT         NOT NULL,
    lang        VARCHAR(16)  NOT NULL DEFAULT 'zh', -- zh, en, vi, ja, ko …
    level       VARCHAR(32),                         -- HSK1-6, N1-N5, CEFR A1-C2 …
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_flashcards_word ON flashcards(word);
CREATE INDEX idx_flashcards_lang ON flashcards(lang);
CREATE INDEX idx_flashcards_level ON flashcards(level);

-- ─────────────────────────────────────────────
-- PER-USER CARD OVERRIDES
-- Users can override front_text, back_text, level for their own view.
-- star, learned, encounter_count are always per-user.
-- ─────────────────────────────────────────────
CREATE TABLE user_card_overrides (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    flashcard_id    UUID NOT NULL REFERENCES flashcards(id) ON DELETE CASCADE,
    front_text      TEXT,           -- NULL = use canonical
    back_text       TEXT,           -- NULL = use canonical
    level           VARCHAR(32),    -- NULL = use canonical
    star            BOOLEAN NOT NULL DEFAULT FALSE,
    learned         BOOLEAN NOT NULL DEFAULT FALSE,
    encounter_count INT     NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, flashcard_id)
);

CREATE INDEX idx_uco_user ON user_card_overrides(user_id);
CREATE INDEX idx_uco_card ON user_card_overrides(flashcard_id);

-- ─────────────────────────────────────────────
-- LEVELS  (lookup table)
-- ─────────────────────────────────────────────
CREATE TABLE levels (
    id          SERIAL PRIMARY KEY,
    code        VARCHAR(32) NOT NULL UNIQUE,   -- e.g. HSK1, N2, A2
    lang        VARCHAR(16) NOT NULL,
    description TEXT,
    sort_order  INT NOT NULL DEFAULT 0
);

INSERT INTO levels (code, lang, sort_order) VALUES
  ('HSK1','zh',1),('HSK2','zh',2),('HSK3','zh',3),
  ('HSK4','zh',4),('HSK5','zh',5),('HSK6','zh',6),
  ('N5','ja',1),('N4','ja',2),('N3','ja',3),('N2','ja',4),('N1','ja',5),
  ('A1','en',1),('A2','en',2),('B1','en',3),('B2','en',4),('C1','en',5),('C2','en',6);

-- ─────────────────────────────────────────────
-- TAGS
-- ─────────────────────────────────────────────
CREATE TABLE tags (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        VARCHAR(64) NOT NULL,
    user_id     UUID REFERENCES users(id) ON DELETE CASCADE, -- NULL = system tag
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(name, user_id)   -- same name can exist per-user
);

CREATE INDEX idx_tags_name ON tags(name);

-- Junction: user tags a flashcard (only visible to that user)
CREATE TABLE flashcard_tags (
    flashcard_id UUID NOT NULL REFERENCES flashcards(id) ON DELETE CASCADE,
    tag_id       UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    word         VARCHAR(255),   -- denormalized for fast search
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (flashcard_id, tag_id, user_id)
);

CREATE INDEX idx_ft_user    ON flashcard_tags(user_id);
CREATE INDEX idx_ft_tag     ON flashcard_tags(tag_id);
CREATE INDEX idx_ft_word    ON flashcard_tags(word);

-- ─────────────────────────────────────────────
-- COMMENTS
-- ─────────────────────────────────────────────
CREATE TABLE comments (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    flashcard_id UUID NOT NULL REFERENCES flashcards(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    word         VARCHAR(255),   -- denormalized for fast search
    body         TEXT NOT NULL,
    is_public    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_comments_card    ON comments(flashcard_id);
CREATE INDEX idx_comments_user    ON comments(user_id);
CREATE INDEX idx_comments_word    ON comments(word);
CREATE INDEX idx_comments_public  ON comments(is_public) WHERE is_public = TRUE;

-- ─────────────────────────────────────────────
-- COLLECTIONS  (sets of flashcards per user)
-- ─────────────────────────────────────────────
CREATE TABLE collections (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID REFERENCES users(id) ON DELETE CASCADE, -- NULL = system/default
    name        VARCHAR(255) NOT NULL,
    description TEXT,
    is_public   BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_collections_user ON collections(user_id);

-- Junction: flashcards in a collection
CREATE TABLE collection_cards (
    collection_id UUID NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    flashcard_id  UUID NOT NULL REFERENCES flashcards(id) ON DELETE CASCADE,
    sort_order    INT  NOT NULL DEFAULT 0,
    added_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (collection_id, flashcard_id)
);

-- ─────────────────────────────────────────────
-- HELPER: auto-update updated_at
-- ─────────────────────────────────────────────
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN NEW.updated_at = NOW(); RETURN NEW; END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_users_updated          BEFORE UPDATE ON users           FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_flashcards_updated     BEFORE UPDATE ON flashcards      FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_uco_updated            BEFORE UPDATE ON user_card_overrides FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_comments_updated       BEFORE UPDATE ON comments        FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trg_collections_updated    BEFORE UPDATE ON collections     FOR EACH ROW EXECUTE FUNCTION set_updated_at();
