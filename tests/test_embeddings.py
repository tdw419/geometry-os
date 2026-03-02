"""Tests for embedding generation."""

import numpy as np
import pytest

from open_brain.embeddings import EMBEDDING_DIM, EmbeddingGenerator


class TestEmbeddingGenerator:
    """Tests for EmbeddingGenerator class."""

    def test_generate_embedding_shape(self):
        """Test that generated embedding has correct 384-dimensional shape."""
        generator = EmbeddingGenerator(backend="local")
        text = "This is a test sentence for embedding."
        embedding = generator.generate(text)

        assert isinstance(embedding, np.ndarray)
        assert embedding.shape == (EMBEDDING_DIM,)
        assert embedding.dtype == np.float32

    def test_similar_texts_have_similar_embeddings(self):
        """Test that semantically similar texts produce similar embeddings."""
        generator = EmbeddingGenerator(backend="local")

        text1 = "The cat sat on the mat."
        text2 = "A cat is sitting on a mat."
        text3 = "Quantum computing enables complex calculations."

        emb1 = generator.generate(text1)
        emb2 = generator.generate(text2)
        emb3 = generator.generate(text3)

        # Similar texts should have high similarity
        sim_similar = EmbeddingGenerator.cosine_similarity(emb1, emb2)
        assert sim_similar > 0.7, f"Similar texts should have similarity > 0.7, got {sim_similar}"

        # Different texts should have lower similarity
        sim_different = EmbeddingGenerator.cosine_similarity(emb1, emb3)
        assert sim_different < sim_similar, (
            f"Different texts should have lower similarity than similar texts"
        )

    def test_batch_generation(self):
        """Test batch embedding generation."""
        generator = EmbeddingGenerator(backend="local")

        texts = [
            "First sentence about cats.",
            "Second sentence about dogs.",
            "Third sentence about birds.",
        ]

        embeddings = generator.generate_batch(texts)

        assert isinstance(embeddings, np.ndarray)
        assert embeddings.shape == (3, EMBEDDING_DIM)
        assert embeddings.dtype == np.float32

        # Each embedding should be non-zero
        for i, emb in enumerate(embeddings):
            assert np.linalg.norm(emb) > 0, f"Embedding {i} should not be zero vector"

    def test_empty_text_returns_zeros(self):
        """Test that empty text returns zero vector."""
        generator = EmbeddingGenerator(backend="local")

        # Test empty string
        embedding_empty = generator.generate("")
        assert embedding_empty.shape == (EMBEDDING_DIM,)
        assert np.allclose(embedding_empty, np.zeros(EMBEDDING_DIM))

        # Test whitespace only
        embedding_whitespace = generator.generate("   ")
        assert embedding_whitespace.shape == (EMBEDDING_DIM,)
        assert np.allclose(embedding_whitespace, np.zeros(EMBEDDING_DIM))

    def test_cosine_similarity_zero_vectors(self):
        """Test cosine similarity with zero vectors."""
        zero = np.zeros(EMBEDDING_DIM)
        nonzero = np.random.randn(EMBEDDING_DIM)

        # Zero vs zero should return 0
        sim = EmbeddingGenerator.cosine_similarity(zero, zero)
        assert sim == 0.0

        # Zero vs nonzero should return 0
        sim = EmbeddingGenerator.cosine_similarity(zero, nonzero)
        assert sim == 0.0

    def test_cosine_similarity_identical_vectors(self):
        """Test cosine similarity of identical vectors."""
        vec = np.random.randn(EMBEDDING_DIM)
        sim = EmbeddingGenerator.cosine_similarity(vec, vec)
        assert np.isclose(sim, 1.0, atol=1e-6)

    def test_batch_generation_empty_list(self):
        """Test batch generation with empty list."""
        generator = EmbeddingGenerator(backend="local")
        embeddings = generator.generate_batch([])
        assert embeddings.shape == (0, EMBEDDING_DIM)

    def test_batch_generation_with_empty_texts(self):
        """Test batch generation containing empty texts."""
        generator = EmbeddingGenerator(backend="local")

        texts = ["Valid text", "", "Another valid text"]
        embeddings = generator.generate_batch(texts)

        assert embeddings.shape == (3, EMBEDDING_DIM)
        # Second embedding should be zero vector
        assert np.allclose(embeddings[1], np.zeros(EMBEDDING_DIM))

    def test_lazy_model_loading(self):
        """Test that model is lazily loaded on first use."""
        generator = EmbeddingGenerator(backend="local")
        assert generator._model is None

        # Model should load on first generate call
        generator.generate("test")
        assert generator._model is not None

    def test_invalid_backend(self):
        """Test that invalid backend raises error."""
        generator = EmbeddingGenerator(backend="invalid_backend")
        with pytest.raises(ValueError, match="Unknown backend"):
            generator.generate("test")
