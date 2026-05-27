-- Actions
CREATE TABLE actions (
    id                UUID        PRIMARY KEY,
    lineage_id        UUID        NOT NULL,
    origin_routine_id UUID,
    title             TEXT        NOT NULL,
    content           TEXT,
    -- ActionState discriminator
    state             TEXT        NOT NULL DEFAULT 'backlogged'
                                  CHECK (state IN ('queued', 'backlogged', 'completed', 'skipped')),
    -- Queued payload: target timestamp + whether it was pinned by the user
    target            TIMESTAMPTZ,
    target_static     BOOLEAN     NOT NULL DEFAULT FALSE,
    -- Backlogged payload: optional soft date hint
    naive_date        DATE,
    -- Completed payload: when the action was marked done
    completed_at      TIMESTAMPTZ,
    duration_secs     BIGINT,
    recurrence_count  INTEGER,
    recurrence_unit   TEXT        CHECK (recurrence_unit IN ('minutes', 'hours', 'days', 'weeks', 'months', 'years')),
    saved             BOOLEAN     NOT NULL DEFAULT TRUE,
    deleted           BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT recurrence_all_or_nothing CHECK (
        (recurrence_count IS NULL) = (recurrence_unit IS NULL)
    ),
    -- State-column consistency
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

-- Events
CREATE TABLE events (
    id               UUID        PRIMARY KEY,
    lineage_id       UUID        NOT NULL,
    title            TEXT        NOT NULL,
    content          TEXT,
    starts_at        TIMESTAMPTZ NOT NULL,
    duration_secs    BIGINT,
    recurrence_count INTEGER,
    recurrence_unit  TEXT        CHECK (recurrence_unit IN ('minutes', 'hours', 'days', 'weeks', 'months', 'years')),
    saved            BOOLEAN     NOT NULL DEFAULT TRUE,
    deleted          BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT recurrence_all_or_nothing CHECK (
        (recurrence_count IS NULL) = (recurrence_unit IS NULL)
    )
);

-- Routines
CREATE TABLE routines (
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
CREATE TABLE routine_steps (
    id            UUID    PRIMARY KEY,
    routine_id    UUID    NOT NULL REFERENCES routines (id) ON DELETE CASCADE,
    title         TEXT    NOT NULL,
    duration_secs BIGINT,
    position      INTEGER NOT NULL
);

CREATE INDEX routine_steps_routine_id_idx ON routine_steps (routine_id);
