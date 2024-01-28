CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE OR REPLACE FUNCTION set_updated_at() RETURNS trigger AS $$
BEGIN
    IF (
        NEW IS DISTINCT FROM OLD AND
        NEW.updated_at IS NOT DISTINCT FROM OLD.updated_at
    ) THEN
        NEW.updated_at := current_timestamp;
    END IF;
    RETURN NEW;
END
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION manage_updated_at(_tbl regclass) RETURNS VOID AS $$
BEGIN
  EXECUTE format('CREATE TRIGGER set_updated_at BEFORE UPDATE ON %s FOR EACH ROW EXECUTE PROCEDURE set_updated_at()', _tbl);
END;
$$ LANGUAGE plpgsql;

CREATE TABLE directories (
    id UUID PRIMARY KEY NOT NULL DEFAULT uuid_generate_v4(),

    name TEXT NOT NULL,

    parent UUID REFERENCES directories(id) ON DELETE CASCADE ON UPDATE CASCADE,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE documents (
    id UUID PRIMARY KEY NOT NULL DEFAULT uuid_generate_v4(),
    
    -- With extension
    file_name TEXT NOT NULL, 
    directory UUID NOT NULL REFERENCES directories(id) ON DELETE CASCADE ON UPDATE CASCADE, 

    -- Markdown content
    content TEXT NOT NULL, 

    -- Meta

    title TEXT,
    -- Minutes
    reading_time INT, 
    tags TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

SELECT manage_updated_at('directories');
SELECT manage_updated_at('documents');