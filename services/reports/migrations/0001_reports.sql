BEGIN;

CREATE SCHEMA reports;
REVOKE ALL ON SCHEMA reports FROM PUBLIC;

CREATE TABLE reports.usage_days (
    report_day date NOT NULL,
    installation_id uuid NOT NULL,
    summary text NOT NULL CHECK (octet_length(summary) <= 4096 AND jsonb_typeof(summary::jsonb) = 'object'),
    PRIMARY KEY (report_day, installation_id)
);

CREATE TABLE reports.usage_intake_quota (
    received_day date PRIMARY KEY,
    accepted_count integer NOT NULL CHECK (accepted_count > 0)
);

CREATE TABLE reports.usage_rate_limits (
    installation_id uuid PRIMARY KEY,
    window_minute bigint NOT NULL,
    request_count integer NOT NULL CHECK (request_count > 0)
);

CREATE FUNCTION reports.save_usage_day(
    target_day date, target_installation uuid, incoming_summary text, receipt_day date, daily_limit integer
) RETURNS text
LANGUAGE plpgsql
SET search_path = pg_catalog
AS $function$
DECLARE
    stored_summary text;
BEGIN
    -- Serialize admissions on the receipt day, including across server instances.
    PERFORM pg_advisory_xact_lock(hashtext('pennypinch.reports.intake'), receipt_day - DATE '2000-01-01');

    IF COALESCE((SELECT accepted_count FROM reports.usage_intake_quota WHERE received_day = receipt_day), 0)
            < daily_limit THEN
        INSERT INTO reports.usage_days (report_day, installation_id, summary)
        VALUES (target_day, target_installation, incoming_summary)
        ON CONFLICT (report_day, installation_id) DO NOTHING;
        IF FOUND THEN
            INSERT INTO reports.usage_intake_quota AS quota (received_day, accepted_count)
            VALUES (receipt_day, 1)
            ON CONFLICT (received_day) DO UPDATE SET accepted_count = quota.accepted_count + 1;
            RETURN 'stored';
        END IF;
    END IF;

    SELECT summary INTO stored_summary FROM reports.usage_days
    WHERE report_day = target_day AND installation_id = target_installation;
    IF stored_summary IS NULL THEN RETURN 'daily_quota'; END IF;
    IF stored_summary = incoming_summary THEN RETURN 'duplicate'; END IF;
    RETURN 'conflict';
END;
$function$;

REVOKE ALL ON FUNCTION reports.save_usage_day(date, uuid, text, date, integer) FROM PUBLIC;

COMMIT;
