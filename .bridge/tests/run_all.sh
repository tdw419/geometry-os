#!/usr/bin/env bash
# .bridge/tests/run_all.sh

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║        CLAUDE-GEMINI BRIDGE TEST SUITE                       ║"
echo "╚══════════════════════════════════════════════════════════════╝"

chmod +x .bridge/tests/*.sh

# Run all test levels
./.bridge/tests/test_components.sh && echo "✓ Component tests passed"
./.bridge/tests/test_integration.sh && echo "✓ Integration tests passed"
# E2E and External Observer tests are commented out by default as they require real CLIs/Network
# ./.bridge/tests/test_e2e.sh && echo "✓ E2E tests passed"
# ./.bridge/tests/test_external_observer.sh && echo "✓ External verification passed"

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║                    TEST SUITE COMPLETED                      ║"
echo "╚══════════════════════════════════════════════════════════════╝"
