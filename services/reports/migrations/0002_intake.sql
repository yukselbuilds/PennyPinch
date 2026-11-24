CREATE TABLE usage_intake_quota (
    received_day TEXT NOT NULL CHECK (length(received_day) = 10),
    accepted_count INTEGER NOT NULL CHECK (accepted_count > 0),
    PRIMARY KEY (received_day)
) STRICT, WITHOUT ROWID;

ALTER TABLE usage_days ADD COLUMN received_day TEXT CHECK (length(received_day) = 10);

CREATE TRIGGER usage_days_count_intake AFTER INSERT ON usage_days
WHEN NEW.received_day IS NOT NULL
BEGIN
    INSERT INTO usage_intake_quota (received_day, accepted_count)
    VALUES (NEW.received_day, 1)
    ON CONFLICT(received_day) DO UPDATE SET accepted_count = accepted_count + 1;
END;
