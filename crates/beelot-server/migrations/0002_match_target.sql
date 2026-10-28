-- The points each match was played to (US-062); matches recorded before were played to 1,000.
ALTER TABLE match_record ADD COLUMN target INTEGER NOT NULL DEFAULT 1000;
