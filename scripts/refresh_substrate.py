#!/usr/bin/env python3
"""
Refresh the SPIR-V Memory Substrate

Exports current memories from PostgreSQL to a SPIR-V binary
for real-time visualization in the Memory Browser.

Usage:
    python3 scripts/refresh_substrate.py
    python3 scripts/refresh_substrate.py --output custom.spv
    python3 scripts/refresh_substrate.py --watch  # Auto-refresh on DB changes
"""

import argparse
import logging
import os
import sys
import time
from pathlib import Path

# Add project root to path
PROJECT_ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

from open_brain.agent_bridge import get_bridge, refresh_substrate

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
logger = logging.getLogger(__name__)


def main():
    parser = argparse.ArgumentParser(description="Refresh SPIR-V Memory Substrate")
    parser.add_argument("--output", "-o", type=str, help="Output .spv file path")
    parser.add_argument("--watch", "-w", action="store_true", help="Watch for changes and auto-refresh")
    parser.add_argument("--interval", "-i", type=int, default=5, help="Watch interval in seconds")
    parser.add_argument("--verbose", "-v", action="store_true", help="Verbose output")
    args = parser.parse_args()

    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)

    output_path = args.output
    if output_path:
        output_path = Path(output_path)

    if args.watch:
        logger.info(f"Starting watch mode (interval: {args.interval}s)")
        logger.info("Press Ctrl+C to stop")

        last_count = 0
        bridge = get_bridge()

        try:
            while True:
                try:
                    # Check for new memories
                    # For now, just refresh periodically
                    result = bridge.refresh_substrate(output_path)
                    if result:
                        logger.info(f"Substrate refreshed: {result}")
                    time.sleep(args.interval)
                except Exception as e:
                    logger.error(f"Refresh error: {e}")
                    time.sleep(args.interval)
        except KeyboardInterrupt:
            logger.info("Watch mode stopped")
    else:
        # Single refresh
        logger.info("Refreshing substrate...")
        result = refresh_substrate(output_path)
        if result:
            logger.info(f"Substrate exported to: {result}")
        else:
            logger.warning("No memories found to export")
            sys.exit(1)


if __name__ == "__main__":
    main()
