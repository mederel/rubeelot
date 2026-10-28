-- Whether each match's bids could name tout-atout and sans-atout (US-063); matches recorded before could not.
ALTER TABLE match_record ADD COLUMN all_trumps BOOLEAN NOT NULL DEFAULT FALSE;
