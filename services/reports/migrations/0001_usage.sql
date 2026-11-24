CREATE TABLE usage_days (
    report_day TEXT NOT NULL CHECK (length(report_day) = 10),
    installation_id TEXT NOT NULL CHECK (length(installation_id) = 36),
    summary TEXT NOT NULL CHECK (length(summary) <= 4096 AND json_type(summary) = 'object'),
    PRIMARY KEY (report_day, installation_id)
) STRICT, WITHOUT ROWID;
