\set ON_ERROR_STOP on

-- Run as the retained PostgreSQL administrator while connected to `postgres`.
-- The wrapper supplies CENTAUR_OS_APP_PASSWORD through the process environment.
-- \getenv keeps the credential out of argv, SQL source, and command output.
\getenv centaur_os_password CENTAUR_OS_APP_PASSWORD
\getenv centaur_os_database CENTAUR_OS_DATABASE_NAME

SELECT format(
    'CREATE ROLE centaur_os_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT PASSWORD %L',
    :'centaur_os_password'
)
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'centaur_os_app')
\gexec

ALTER ROLE centaur_os_app NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;

SELECT format('CREATE DATABASE %I OWNER centaur_os_app', :'centaur_os_database')
WHERE NOT EXISTS (
    SELECT 1 FROM pg_database WHERE datname = :'centaur_os_database'
)
\gexec

SELECT format('ALTER DATABASE %I OWNER TO centaur_os_app', :'centaur_os_database')
\gexec

SELECT format('REVOKE ALL ON DATABASE %I FROM PUBLIC', :'centaur_os_database')
\gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO centaur_os_app', :'centaur_os_database')
\gexec

\connect :centaur_os_database
CREATE EXTENSION IF NOT EXISTS vector;
ALTER SCHEMA public OWNER TO centaur_os_app;
REVOKE ALL ON SCHEMA public FROM PUBLIC;
GRANT USAGE, CREATE ON SCHEMA public TO centaur_os_app;
