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
    path TEXT NOT NULL,
    parent UUID REFERENCES directories(id) ON DELETE CASCADE ON UPDATE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE documents (
    id UUID PRIMARY KEY NOT NULL DEFAULT uuid_generate_v4(),
    file_name TEXT NOT NULL, -- With extension
    directory UUID NOT NULL REFERENCES directories(id) ON DELETE CASCADE ON UPDATE CASCADE, 
    path TEXT NOT NULL,
    title TEXT, -- Temporary title obtained from h1, overriden by meta title
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

SELECT manage_updated_at('directories');
SELECT manage_updated_at('documents');

CREATE OR REPLACE FUNCTION update_paths_recursive() RETURNS trigger AS $$
BEGIN
    IF (
        NEW.path IS DISTINCT FROM OLD.path
    ) THEN
        WITH RECURSIVE 
        dirs AS 
            (SELECT id 
            FROM directories 
            WHERE id =  NEW.id
            UNION ALL 
            SELECT d.id FROM directories d 
            INNER JOIN dirs
            ON d.parent = dirs.id) 
        UPDATE directories SET path = REPLACE(path, NEW.path, 'foo') 
        WHERE id IN (SELECT id FROM dirs);
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE TRIGGER update_path AFTER UPDATE OF path ON directories FOR EACH ROW EXECUTE FUNCTION update_paths_recursive();