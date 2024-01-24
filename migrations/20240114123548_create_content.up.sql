CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE documents (
    id UUID PRIMARY KEY NOT NULL DEFAULT uuid_generate_v4(),
    file_name TEXT NOT NULL, -- With extension
    root_dir TEXT NOT NULL, -- Origin directory
    content TEXT NOT NULL, -- Markdown content

    -- Meta

    title TEXT,
    reading_time INT, -- Minutes
    tags TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
