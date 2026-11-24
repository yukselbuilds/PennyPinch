BEGIN;

CREATE ROLE pennypinch_reports_runtime NOLOGIN;
ALTER ROLE pennypinch_reports_runtime SET statement_timeout = '8s';

GRANT USAGE ON SCHEMA reports TO pennypinch_reports_runtime;
GRANT SELECT, INSERT ON reports.usage_days TO pennypinch_reports_runtime;
GRANT SELECT, INSERT, UPDATE (accepted_count)
    ON reports.usage_intake_quota TO pennypinch_reports_runtime;
GRANT SELECT, INSERT, UPDATE (window_minute, request_count)
    ON reports.usage_rate_limits TO pennypinch_reports_runtime;
GRANT EXECUTE ON FUNCTION reports.save_usage_day(date, uuid, text, date, integer)
    TO pennypinch_reports_runtime;

COMMIT;
