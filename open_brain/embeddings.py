"""Embedding generation for Open Brain with multiple backend support."""

import logging
from typing import Optional

import numpy as np

logger = logging.getLogger(__name__)

# Default embedding dimension for all-MiniLM-L6-v2
EMBEDDING_DIM = 384


class EmbeddingGenerator:
    """Generate embeddings using local or LM Studio backends.

    Supports lazy loading of models to avoid memory overhead when not in use.
    """

    def __init__(
        self,
        backend: str = "local",
        model_name: str = "all-MiniLM-L6-v2",
        lm_studio_url: Optional[str] = None,
    ):
        """Initialize embedding generator.

        Args:
            backend: "local" for sentence-transformers, "lm_studio" for LM Studio API
            model_name: Model name for local backend
            lm_studio_url: URL for LM Studio API (e.g., "http://localhost:1234")
        """
        self.backend = backend
        self.model_name = model_name
        self.lm_studio_url = lm_studio_url or "http://localhost:1234"
        self._model = None

    def _load_model(self):
        """Lazy load the sentence-transformers model."""
        if self._model is None and self.backend == "local":
            try:
                from sentence_transformers import SentenceTransformer

                logger.info(f"Loading embedding model: {self.model_name}")
                self._model = SentenceTransformer(self.model_name)
            except ImportError:
                raise ImportError(
                    "sentence-transformers not installed. "
                    "Install with: pip install sentence-transformers"
                )
        return self._model

    def generate(self, text: str) -> np.ndarray:
        """Generate embedding for a single text.

        Args:
            text: Input text to embed

        Returns:
            384-dimensional numpy array
        """
        if not text or not text.strip():
            return np.zeros(EMBEDDING_DIM, dtype=np.float32)

        if self.backend == "local":
            model = self._load_model()
            embedding = model.encode(text, convert_to_numpy=True)
            return embedding.astype(np.float32)

        elif self.backend == "lm_studio":
            return self._generate_lm_studio(text)

        else:
            raise ValueError(f"Unknown backend: {self.backend}")

    def generate_batch(self, texts: list[str]) -> np.ndarray:
        """Generate embeddings for multiple texts.

        Args:
            texts: List of input texts

        Returns:
            2D numpy array of shape (len(texts), 384)
        """
        if not texts:
            return np.array([], dtype=np.float32).reshape(0, EMBEDDING_DIM)

        # Handle empty strings in batch
        results = []
        for text in texts:
            if not text or not text.strip():
                results.append(np.zeros(EMBEDDING_DIM, dtype=np.float32))
            else:
                results.append(self.generate(text))

        return np.array(results, dtype=np.float32)

    def _generate_lm_studio(self, text: str) -> np.ndarray:
        """Generate embedding using LM Studio API.

        Args:
            text: Input text

        Returns:
            384-dimensional numpy array
        """
        try:
            import requests
        except ImportError:
            raise ImportError(
                "requests not installed. Install with: pip install requests"
            )

        url = f"{self.lm_studio_url}/v1/embeddings"
        payload = {"input": text, "model": self.model_name}

        try:
            response = requests.post(url, json=payload, timeout=30)
            response.raise_for_status()
            data = response.json()
            embedding = data["data"][0]["embedding"]
            return np.array(embedding, dtype=np.float32)
        except requests.exceptions.RequestException as e:
            logger.error(f"LM Studio API error: {e}")
            raise RuntimeError(f"LM Studio API error: {e}") from e

    @staticmethod
    def cosine_similarity(a: np.ndarray, b: np.ndarray) -> float:
        """Calculate cosine similarity between two vectors.

        Args:
            a: First vector
            b: Second vector

        Returns:
            Cosine similarity score between -1 and 1
        """
        norm_a = np.linalg.norm(a)
        norm_b = np.linalg.norm(b)

        if norm_a == 0 or norm_b == 0:
            return 0.0

        return float(np.dot(a, b) / (norm_a * norm_b))
