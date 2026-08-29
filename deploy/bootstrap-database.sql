\set ON_ERROR_STOP on

-- Run as the retained PostgreSQL administrator while connected to `postgres`.
-- The wrapper supplies CENTAUR_CONTEXT_APP_PASSWORD through the process environment.
-- \getenv keeps the credential out of argv, SQL source, and command output.
\getenv centaur_context_password CENTAUR_CONTEXT_APP_PASSWORD
\getenv centaur_context_database CENTAUR_CONTEXT_DATABASE_NAME

SELECT format(
    'CREATE ROLE centaur_context_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT PASSWORD %L',
    :'centaur_context_password'
)
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'centaur_context_app')
\gexec

ALTER ROLE centaur_context_app NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT;

SELECT format('CREATE DATABASE %I OWNER centaur_context_app', :'centaur_context_database')
WHERE NOT EXISTS (
    SELECT 1 FROM pg_database WHERE datname = :'centaur_context_database'
)
\gexec

SELECT format('ALTER DATABASE %I OWNER TO centaur_context_app', :'centaur_context_database')
\gexec

SELECT format('REVOKE ALL ON DATABASE %I FROM PUBLIC', :'centaur_context_database')
\gexec
SELECT format('GRANT CONNECT ON DATABASE %I TO centaur_context_app', :'centaur_context_database')
\gexec

\connect :centaur_context_database
CREATE EXTENSION IF NOT EXISTS vector;
ALTER SCHEMA public OWNER TO centaur_context_app;
REVOKE ALL ON SCHEMA public FROM PUBLIC;
GRANT USAGE, CREATE ON SCHEMA public TO centaur_context_app;
