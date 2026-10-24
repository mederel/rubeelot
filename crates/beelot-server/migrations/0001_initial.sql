-- Beelot's schema. Times are DATETIME(6) in UTC; identifiers are MariaDB UUIDs.

-- A player with a passkey, known only by a random id and a pseudonym.
CREATE TABLE account (
    id                  UUID        NOT NULL PRIMARY KEY,
    display_name        VARCHAR(30) NOT NULL,
    created_at          DATETIME(6) NOT NULL,
    last_sign_in_at     DATETIME(6) NOT NULL,
    -- The sign-in before the latest one, shown to the player to spot an access they did not make.
    previous_sign_in_at DATETIME(6)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

-- An account's one passkey: the serialized webauthn-rs Passkey.
CREATE TABLE passkey (
    account_id    UUID            NOT NULL PRIMARY KEY,
    credential_id VARBINARY(1023) NOT NULL,
    passkey       JSON            NOT NULL,
    created_at    DATETIME(6)     NOT NULL,
    last_used_at  DATETIME(6),
    CONSTRAINT passkey_credential_unique UNIQUE (credential_id),
    CONSTRAINT passkey_account_fk FOREIGN KEY (account_id) REFERENCES account (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

-- A signed-in browser: only the SHA-256 of its cookie's token is kept.
CREATE TABLE session (
    token_hash   BINARY(32)  NOT NULL PRIMARY KEY,
    account_id   UUID        NOT NULL,
    signed_in_at DATETIME(6) NOT NULL,
    last_seen_at DATETIME(6) NOT NULL,
    INDEX session_account_idx (account_id),
    CONSTRAINT session_account_fk FOREIGN KEY (account_id) REFERENCES account (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

-- Finished matches with at least one signed-in player. Teams are NORTH_SOUTH or EAST_WEST.
CREATE TABLE match_record (
    id                UUID        NOT NULL PRIMARY KEY,
    ended_at          DATETIME(6) NOT NULL,
    mode              VARCHAR(16) NOT NULL,
    variant           VARCHAR(16) NOT NULL,
    difficulty        VARCHAR(16),
    north_south_score INTEGER     NOT NULL,
    east_west_score   INTEGER     NOT NULL,
    winning_team      VARCHAR(16) NOT NULL,
    announcements     BOOLEAN     NOT NULL DEFAULT FALSE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

-- The four seats in play order (position 0 is North); account_id is null for guests, bots and deleted accounts.
CREATE TABLE match_seat (
    match_id   UUID        NOT NULL,
    position   INTEGER     NOT NULL,
    account_id UUID,
    name       VARCHAR(30) NOT NULL,
    team       VARCHAR(16) NOT NULL,
    bot        BOOLEAN     NOT NULL,
    PRIMARY KEY (match_id, position),
    INDEX match_seat_account_idx (account_id),
    CONSTRAINT match_seat_match_fk FOREIGN KEY (match_id) REFERENCES match_record (id) ON DELETE CASCADE,
    CONSTRAINT match_seat_account_fk FOREIGN KEY (account_id) REFERENCES account (id) ON DELETE SET NULL
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

-- Each round in the order played; contract_value is null in classic Belote.
CREATE TABLE match_round (
    match_id                  UUID        NOT NULL,
    round_index               INTEGER     NOT NULL,
    declaring_team            VARCHAR(16) NOT NULL,
    trump                     VARCHAR(16) NOT NULL,
    contract_value            INTEGER,
    coinched                  BOOLEAN     NOT NULL,
    contract_made             BOOLEAN     NOT NULL,
    capot_team                VARCHAR(16),
    belote_team               VARCHAR(16),
    north_south_points        INTEGER     NOT NULL,
    east_west_points          INTEGER     NOT NULL,
    north_south_announcements INTEGER     NOT NULL DEFAULT 0,
    east_west_announcements   INTEGER     NOT NULL DEFAULT 0,
    PRIMARY KEY (match_id, round_index),
    CONSTRAINT match_round_match_fk FOREIGN KEY (match_id) REFERENCES match_record (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

-- Security events: never a token. account_id has no foreign key, so the history never blocks a deletion; an
-- account's events go with it.
CREATE TABLE security_event (
    id          BIGINT      NOT NULL AUTO_INCREMENT PRIMARY KEY,
    account_id  UUID,
    type        VARCHAR(32) NOT NULL,
    outcome     VARCHAR(16) NOT NULL,
    ip_address  VARCHAR(45),
    occurred_at DATETIME(6) NOT NULL,
    INDEX security_event_account_idx (account_id, occurred_at),
    INDEX security_event_occurred_idx (occurred_at)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;
