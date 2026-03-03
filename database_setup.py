#!/usr/bin/env python3
import psycopg2
import sys

def setup_database():
    """Set up PostgreSQL database with pgvector for Open Brain integration"""
    try:
        # Connect to PostgreSQL (adjust connection string as needed)
        conn = psycopg2.connect(
            dbname="openbrain",
            user="postgres",
            password="",
            host="localhost",
            port="5432"
        )
        cur = conn.cursor()
        
        # Enable pgvector extension
        cur.execute("CREATE EXTENSION IF NOT EXISTS vector;")
        
        # Create memory schema with vector support
        cur.execute("""
        CREATE SCHEMA IF NOT EXISTS memories;
        SET search_path = memories;
        
        -- Main memory table
        CREATE TABLE IF NOT EXISTS memory_entries (
            id SERIAL PRIMARY KEY,
            type VARCHAR(50) NOT NULL,
            content JSONB NOT NULL,
            embedding VECTOR(384),
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            confidence DOUBLE PRECISION DEFAULT 1.0,
            valid_from TIMESTAMP,
            valid_to TIMESTAMP,
            tags TEXT[] DEFAULT '{}',
            metadata JSONB DEFAULT '{}'
        );
        
        -- Create index for vector search
        CREATE INDEX IF NOT EXISTS idx_memory_embeddings 
        ON memory_entries 
        USING ivfflat (embedding vector_cosine_ops)
        WITH (lists = 100);
        
        -- Create index for text search
        CREATE INDEX IF NOT EXISTS idx_memory_content 
        ON memory_entries 
        USING GIN (content jsonb_path_ops);
        
        -- Create index for tags
        CREATE INDEX IF NOT EXISTS idx_memory_tags 
        ON memory_entries 
        USING GIN (tags);
        
        -- Create index for temporal queries
        CREATE INDEX IF NOT EXISTS idx_memory_temporal 
        ON memory_entries (valid_from, valid_to);
        
        -- Create table for glyph atlas data
        CREATE TABLE IF NOT EXISTS glyph_atlas (
            id SERIAL PRIMARY KEY,
            memory_id INTEGER REFERENCES memory_entries(id),
            atlas_data BYTEA,
            width INTEGER,
            height INTEGER,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        
        -- Create table for spatial encoding
        CREATE TABLE IF NOT EXISTS spatial_encoding (
            id SERIAL PRIMARY KEY,
            memory_id INTEGER REFERENCES memory_entries(id),
            hilbert_index INTEGER,
            x_position INTEGER,
            y_position INTEGER,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        
        -- Create table for visual rendering metadata
        CREATE TABLE IF NOT EXISTS visual_metadata (
            id SERIAL PRIMARY KEY,
            memory_id INTEGER REFERENCES memory_entries(id),
            rgb_encoding JSONB,
            symmetry_type VARCHAR(20),
            visual_density DOUBLE PRECISION,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        
        COMMIT;
        print("Database setup completed successfully!")
        
        except Exception as e:
        print(f"Error setting up database: {e}")
        conn.rollback()
    finally:
        if conn:
            conn.close()

if __name__ == "__main__":
    setup_database()