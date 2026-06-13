CREATE TABLE IF NOT EXISTS actions (
    id                UUID        PRIMARY KEY,
    lineage_id        UUID        NOT NULL,
    routine_id        UUID,
    template_id       UUID,
    title             TEXT        NOT NULL,
    content           TEXT,
    state             TEXT        NOT NULL DEFAULT 'backlogged'
                                  CHECK (state IN ('queued', 'backlogged', 'completed', 'skipped')),
    target            TIMESTAMPTZ,
    target_static     BOOLEAN     NOT NULL DEFAULT FALSE,
    naive_date        DATE,
    completed_at      TIMESTAMPTZ,
    duration_secs     BIGINT,
    recurrence_count  INTEGER,
    recurrence_unit   TEXT        CHECK (recurrence_unit IN ('minutes', 'hours', 'days', 'weeks', 'months', 'years')),
    deleted           BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT recurrence_all_or_nothing CHECK (
        (recurrence_count IS NULL) = (recurrence_unit IS NULL)
    ),
    CONSTRAINT queued_requires_target CHECK (
        state != 'queued' OR target IS NOT NULL
    ),
    CONSTRAINT target_only_when_queued CHECK (
        state = 'queued' OR target IS NULL
    ),
    CONSTRAINT naive_date_only_when_backlogged CHECK (
        state = 'backlogged' OR naive_date IS NULL
    ),
    CONSTRAINT completed_at_requires_completed_state CHECK (
        state != 'completed' OR completed_at IS NOT NULL
    ),
    CONSTRAINT completed_at_only_when_completed CHECK (
        state = 'completed' OR completed_at IS NULL
    )
);

CREATE TABLE IF NOT EXISTS action_templates (
    id                UUID        PRIMARY KEY,
    lineage_id        UUID        NOT NULL,
    title             TEXT        NOT NULL,
    content           TEXT,
    duration_secs     BIGINT,
    recurrence_count  INTEGER,
    recurrence_unit   TEXT        CHECK (recurrence_unit IN ('minutes', 'hours', 'days', 'weeks', 'months', 'years')),
    deleted           BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT recurrence_all_or_nothing CHECK (
        (recurrence_count IS NULL) = (recurrence_unit IS NULL)
    )
);

CREATE TABLE IF NOT EXISTS events (
    id               UUID        PRIMARY KEY,
    lineage_id       UUID        NOT NULL,
    template_id      UUID,
    title            TEXT        NOT NULL,
    content          TEXT,
    starts_at        TIMESTAMPTZ NOT NULL,
    duration_secs    BIGINT      NOT NULL DEFAULT 3600,
    recurrence_count INTEGER,
    recurrence_unit  TEXT        CHECK (recurrence_unit IN ('minutes', 'hours', 'days', 'weeks', 'months', 'years')),
    deleted          BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT recurrence_all_or_nothing CHECK (
        (recurrence_count IS NULL) = (recurrence_unit IS NULL)
    )
);

CREATE TABLE IF NOT EXISTS event_templates (
    id               UUID        PRIMARY KEY,
    lineage_id       UUID        NOT NULL,
    title            TEXT        NOT NULL,
    content          TEXT,
    duration_secs    BIGINT,
    recurrence_count INTEGER,
    recurrence_unit  TEXT        CHECK (recurrence_unit IN ('minutes', 'hours', 'days', 'weeks', 'months', 'years')),
    deleted          BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT recurrence_all_or_nothing CHECK (
        (recurrence_count IS NULL) = (recurrence_unit IS NULL)
    )
);

-- Routines
CREATE TABLE IF NOT EXISTS routines (
    id               UUID        PRIMARY KEY,
    title            TEXT        NOT NULL,
    content          TEXT,
    target           TIMESTAMPTZ,
    recurrence_count INTEGER,
    recurrence_unit  TEXT        CHECK (recurrence_unit IN ('minutes', 'hours', 'days', 'weeks', 'months', 'years')),
    deleted          BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT recurrence_all_or_nothing CHECK (
        (recurrence_count IS NULL) = (recurrence_unit IS NULL)
    )
);

-- Routine steps (ordered by position)
CREATE TABLE IF NOT EXISTS routine_steps (
    id            UUID    PRIMARY KEY,
    routine_id    UUID    NOT NULL REFERENCES routines (id) ON DELETE CASCADE,
    title         TEXT    NOT NULL,
    duration_secs BIGINT,
    position      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS routine_steps_routine_id_idx ON routine_steps (routine_id);
