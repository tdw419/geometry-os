#!/usr/bin/env node
/**
 * Geometry OS Build Script
 *
 * Packages the OS for distribution:
 * 1. Validates all JS files
 * 2. Creates dist directory
 * 3. Copies all necessary files
 * 4. Generates distribution manifest
 */

import { readdir, readFile, writeFile, mkdir, cp, stat } from 'fs/promises';
import { join, basename } from 'path';
import { execSync } from 'child_process';
import { createHash } from 'crypto';

const ROOT = join(import.meta.dirname, '..');
const WEB_SRC = join(ROOT, 'web');
const DIST = join(ROOT, 'dist');

// Files to include in distribution
const INCLUDE_PATTERNS = [
  '*.js',
  '*.wgsl',
  '*.html',
  '*.json'
];

// Core files (always included)
const CORE_FILES = [
  'GeometryKernel.js',
  'GPUMemoryManager.js',
  'Shell.js',
  'GOSRouter.js',
  'VisualDesktop.js',
  'Process.js',
  'Scheduler.js',
  'VisualShell.js',
  'kernel.wgsl',
  'Profiler.js',
  'CompilerAgent.js',
  'compiler.wgsl',
  'WatchdogAgent.js',
  'watchdog.wgsl',
  'CognitiveAgent.js'
];

// Test pages (included for demo)
const TEST_PAGES = [
  'test-memory.html',
  'test-filesystem.html',
  'test-shell.html',
  'test-network.html',
  'test-visual-desktop.html',
  'test-integration.html',
  'test-gpu-compiler.html',
  'test-watchdog.html',
  'test-cognitive.html',
  'test-selfhost-compiler.html',
  'test-profiler.html'
];

// Asset files
const ASSETS = [
  'assets/glyph_info.json'
];

async function fileExists(path) {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

async function validateJS(filepath) {
  try {
    execSync(`node --check "${filepath}"`, { stdio: 'pipe' });
    return true;
  } catch (e) {
    console.error(`  ✗ Syntax error in ${basename(filepath)}: ${e.stderr?.toString() || e.message}`);
    return false;
  }
}

async function getFileHash(filepath) {
  const content = await readFile(filepath);
  return createHash('sha256').update(content).digest('hex').slice(0, 16);
}

async function build() {
  console.log('\n========================================');
  console.log('Geometry OS Build Script');
  console.log('========================================\n');

  // Step 1: Validate JS files
  console.log('[1/4] Validating JavaScript files...');
  let validCount = 0;
  let invalidCount = 0;

  const jsFiles = CORE_FILES.filter(f => f.endsWith('.js'));
  for (const file of jsFiles) {
    const filepath = join(WEB_SRC, file);
    if (await fileExists(filepath)) {
      if (await validateJS(filepath)) {
        console.log(`  ✓ ${file}`);
        validCount++;
      } else {
        invalidCount++;
      }
    } else {
      console.log(`  ⚠ ${file} not found`);
    }
  }

  if (invalidCount > 0) {
    console.error(`\n✗ Build failed: ${invalidCount} file(s) have syntax errors`);
    process.exit(1);
  }
  console.log(`  ${validCount} files validated\n`);

  // Step 2: Create dist directory
  console.log('[2/4] Creating distribution directory...');
  await mkdir(join(DIST, 'web', 'assets'), { recursive: true });
  await mkdir(join(DIST, 'docs'), { recursive: true });
  console.log('  ✓ dist/ created\n');

  // Step 3: Copy files
  console.log('[3/4] Copying files...');

  const manifest = {
    version: '1.0.0',
    buildDate: new Date().toISOString(),
    files: {},
    core: {},
    tests: {},
    assets: {}
  };

  // Copy core files
  for (const file of CORE_FILES) {
    const src = join(WEB_SRC, file);
    if (await fileExists(src)) {
      const dest = join(DIST, 'web', file);
      await cp(src, dest);
      const hash = await getFileHash(dest);
      manifest.core[file] = hash;
      console.log(`  ✓ web/${file}`);
    }
  }

  // Copy test pages
  for (const file of TEST_PAGES) {
    const src = join(WEB_SRC, file);
    if (await fileExists(src)) {
      const dest = join(DIST, 'web', file);
      await cp(src, dest);
      const hash = await getFileHash(dest);
      manifest.tests[file] = hash;
      console.log(`  ✓ web/${file}`);
    }
  }

  // Copy assets
  for (const file of ASSETS) {
    const src = join(WEB_SRC, file);
    if (await fileExists(src)) {
      const dest = join(DIST, 'web', file);
      await mkdir(join(DIST, 'web', 'assets'), { recursive: true });
      await cp(src, dest);
      const hash = await getFileHash(dest);
      manifest.assets[file] = hash;
      console.log(`  ✓ web/${file}`);
    }
  }

  // Copy docs
  const docsDir = join(ROOT, 'docs');
  if (await fileExists(docsDir)) {
    const docs = await readdir(docsDir);
    for (const doc of docs) {
      if (doc.endsWith('.md')) {
        const src = join(docsDir, doc);
        const dest = join(DIST, 'docs', doc);
        await cp(src, dest);
        console.log(`  ✓ docs/${doc}`);
      }
    }
  }

  // Copy package.json
  await cp(join(ROOT, 'package.json'), join(DIST, 'package.json'));
  console.log('  ✓ package.json');

  // Copy README
  if (await fileExists(join(ROOT, 'README.md'))) {
    await cp(join(ROOT, 'README.md'), join(DIST, 'README.md'));
    console.log('  ✓ README.md');
  }

  console.log('');

  // Step 4: Generate manifest
  console.log('[4/4] Generating manifest...');
  manifest.files = {
    core: Object.keys(manifest.core).length,
    tests: Object.keys(manifest.tests).length,
    assets: Object.keys(manifest.assets).length
  };

  await writeFile(join(DIST, 'manifest.json'), JSON.stringify(manifest, null, 2));
  console.log('  ✓ manifest.json\n');

  // Summary
  console.log('========================================');
  console.log('Build Complete!');
  console.log('========================================');
  console.log(`  Core files: ${manifest.files.core}`);
  console.log(`  Test pages: ${manifest.files.tests}`);
  console.log(`  Assets:     ${manifest.files.assets}`);
  console.log(`  Output:     ${DIST}`);
  console.log('');

  // Print usage
  console.log('To serve the distribution:');
  console.log('  cd dist && python3 -m http.server 8770');
  console.log('Then open: http://localhost:8770/web/test-integration.html');
  console.log('');
}

build().catch(err => {
  console.error('Build failed:', err);
  process.exit(1);
});
