\set ON_ERROR_STOP on

-- Run as the retained PostgreSQL administrator while connected to `postgres`.
-- Supply centaur_os_password as a psql variable; never store it in this file.
SELECT format(
    'CREATE ROLE centaur_os_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT PASSWORD %L',
    :'centaur_os_password'
)
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'centaur_os_app')
\gexec

SELECT 'CREATE DATABASE centaur_os OWNER centaur_os_app'
WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = 'centaur_os')
\gexec

REVOKE ALL ON DATABASE centaur_os FROM PUBLIC;
GRANT CONNECT ON DATABASE centaur_os TO centaur_os_app;

\connect centaur_os
CREATE EXTENSION IF NOT EXISTS vector;
ALTER SCHEMA public OWNER TO centaur_os_app;
REVOKE ALL ON SCHEMA public FROM PUBLIC;
GRANT USAGE, CREATE ON SCHEMA public TO centaur_os_app;
