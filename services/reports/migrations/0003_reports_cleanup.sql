BEGIN;

CREATE ROLE pennypinch_reports_cleanup NOLOGIN;
ALTER ROLE pennypinch_reports_cleanup SET statement_timeout = '8s';

GRANT USAGE ON SCHEMA reports TO pennypinch_reports_cleanup;
GRANT DELETE, SELECT (report_day, installation_id)
    ON reports.usage_days TO pennypinch_reports_cleanup;
GRANT DELETE, SELECT (received_day)
    ON reports.usage_intake_quota TO pennypinch_reports_cleanup;
GRANT DELETE, SELECT (window_minute)
    ON reports.usage_rate_limits TO pennypinch_reports_cleanup;

COMMIT;
