/**
 * Prompt Queue Manager - Multi-Provider Rate Limit Handler
 * 
 * Features:
 * - Queues prompts when rate limited
 * - Routes to multiple providers (Anthropic, OpenAI, Local)
 * - Exponential backoff per provider
 * - Persists queue state across sessions
 * - Automatic failover on 429 errors
 */

import { writeFileSync, readFileSync, existsSync, mkdirSync } from 'fs';
import { join } from 'path';
import { EventEmitter } from 'events';

const STATE_DIR = join(process.env.PROJECT_ROOT || '.', '.ouroboros', 'queue');
const QUEUE_FILE = join(STATE_DIR, 'prompt_queue.json');
const RATE_LIMIT_FILE = join(STATE_DIR, 'rate_limits.json');

mkdirSync(STATE_DIR, { recursive: true });

// Provider configuration - based on actual subscriptions
// See: /home/jericho/zion/docs/research/453_self_prompting_rate_limits.txt
const PROVIDERS = {
    glm: {
        name: 'GLM Coding Max',
        envKey: 'ZAI_API_KEY',
        baseUrl: 'https://api.z.ai/api/coding/paas/v4',
        promptsPerWindow: 2400,  // per 5 hours
        windowMs: 5 * 60 * 60 * 1000,  // 5 hours in ms
        contextTokens: 100000,
        outputTokens: 4096,
        model: 'glm-4-plus',  // or glm-5-turbo
        priority: 1,  // Highest capacity, use first
        enabled: true,
        auth: 'api_key',
        notes: '$216/quarter, 2400 prompts/5hr, best for heavy use'
    },
    gemini: {
        name: 'Gemini Code Assist',
        // OAuth-based - use via CLI tool, not direct API
        auth: 'oauth',
        rpmLimit: 120,
        rpdLimit: 1500,
        model: 'gemini-2.5-pro',
        fallbackModel: 'gemini-2.5-flash',
        priority: 2,
        enabled: true,
        notes: '~$19/mo, 1500 RPD, 120 RPM, requires OAuth/CLI'
    },
    claude: {
        name: 'Claude Pro',
        // OAuth-based - use via Claude CLI, not direct API
        auth: 'oauth',
        promptsPerWindow: 25,  // conservative avg (10-40 range)
        windowMs: 5 * 60 * 60 * 1000,
        model: 'claude-sonnet-4-20250514',
        priority: 3,
        enabled: true,
        notes: '$20/mo, 10-40 prompts/5hr, requires OAuth/CLI'
    },
    local: {
        name: 'Local LM Studio',
        envKey: null,
        auth: 'none',
        rpmLimit: 1000,
        model: 'qwen3.5-27b',  // or whatever model is loaded
        endpoint: 'http://localhost:1234/v1/chat/completions',
        priority: 4,
        enabled: true,
        notes: 'Unlimited local inference via LM Studio'
    }
};

class RateLimitTracker {
    constructor() {
        this.limits = this.load();
    }

    load() {
        if (!existsSync(RATE_LIMIT_FILE)) {
            return {};
        }
        try {
            return JSON.parse(readFileSync(RATE_LIMIT_FILE, 'utf-8'));
        } catch {
            return {};
        }
    }

    save() {
        writeFileSync(RATE_LIMIT_FILE, JSON.stringify(this.limits, null, 2));
    }

    recordRequest(provider, tokens = 0) {
        const now = Date.now();
        if (!this.limits[provider]) {
            this.limits[provider] = { requests: [], tokens: [] };
        }
        
        this.limits[provider].requests.push(now);
        if (tokens > 0) {
            this.limits[provider].tokens.push({ time: now, count: tokens });
        }
        
        this.save();
    }

    recordRateLimit(provider, retryAfter = 60) {
        if (!this.limits[provider]) {
            this.limits[provider] = { requests: [], tokens: [], rateLimited: {} };
        }
        
        this.limits[provider].rateLimited = {
            until: Date.now() + (retryAfter * 1000),
            count: (this.limits[provider].rateLimited?.count || 0) + 1
        };
        
        this.save();
    }

    isRateLimited(provider) {
        if (!this.limits[provider]?.rateLimited) {
            return false;
        }
        
        const { until } = this.limits[provider].rateLimited;
        if (Date.now() < until) {
            return true;
        }
        
        // Clear expired rate limit
        delete this.limits[provider].rateLimited;
        this.save();
        return false;
    }

    getWaitTime(provider) {
        if (!this.limits[provider]?.rateLimited) {
            return 0;
        }
        return Math.max(0, this.limits[provider].rateLimited.until - Date.now());
    }

    getBackoffTime(provider) {
        const count = this.limits[provider]?.rateLimited?.count || 0;
        // Exponential backoff: 2^count seconds, max 5 minutes
        return Math.min(Math.pow(2, count) * 1000, 300000);
    }

    getRecentRequests(provider, windowMs = 60000) {
        const now = Date.now();
        const requests = this.limits[provider]?.requests || [];
        return requests.filter(t => now - t < windowMs).length;
    }

    getRecentDaysRequests(provider) {
        // For Gemini RPD limit - 24 hour window
        return this.getRecentRequests(provider, 24 * 60 * 60 * 1000);
    }

    isProviderAvailable(provider) {
        const config = PROVIDERS[provider];
        if (!config || !config.enabled) return false;
        
        // Check authentication status
        if (config.auth === 'oauth') {
            // Check if OAuth credentials exist
            // Claude stores in ~/.claude/, Gemini stores in ~/.config/gemini/
            const home = process.env.HOME || process.env.USERPROFILE;
            if (provider === 'claude') {
                // Check for Claude credentials
                const claudeCredPath = join(home, '.claude', 'credentials.json');
                if (!existsSync(claudeCredPath)) {
                    this.limits[provider] = this.limits[provider] || {};
                    this.limits[provider].unavailable = 'not logged in';
                    return false;
                }
            } else if (provider === 'gemini') {
                // Check for Gemini credentials
                const geminiCredPath = join(home, '.config', 'gemini', 'credentials.json');
                if (!existsSync(geminiCredPath)) {
                    this.limits[provider] = this.limits[provider] || {};
                    this.limits[provider].unavailable = 'not logged in';
                    return false;
                }
            }
        } else if (config.auth === 'none') {
            // Check if local server is running
            // We'll check this lazily on first request
        } else if (config.envKey) {
            // Check if API key is set
            if (!process.env[config.envKey]) {
                this.limits[provider] = this.limits[provider] || {};
                this.limits[provider].unavailable = 'API key not set';
                return false;
            }
        }
        
        // Clear unavailable flag if we got here
        if (this.limits[provider]?.unavailable) {
            delete this.limits[provider].unavailable;
        }
        
        return true;
    }

    canMakeRequest(provider) {
        const config = PROVIDERS[provider];
        if (!config || !config.enabled) return false;
        if (!this.isProviderAvailable(provider)) return false;
        if (this.isRateLimited(provider)) return false;
        
        // Handle different rate limit types per provider
        if (config.promptsPerWindow) {
            // GLM and Claude: prompts per time window
            const windowMs = config.windowMs || (5 * 60 * 60 * 1000);
            const recent = this.getRecentRequests(provider, windowMs);
            return recent < config.promptsPerWindow * 0.9;  // 90% threshold
        } else if (config.rpdLimit) {
            // Gemini: requests per day
            const daily = this.getRecentDaysRequests(provider);
            if (daily >= config.rpdLimit * 0.9) return false;
            // Also check RPM if set
            if (config.rpmLimit) {
                const recent = this.getRecentRequests(provider, 60000);
                return recent < config.rpmLimit * 0.9;
            }
            return true;
        } else if (config.rpmLimit) {
            // Local: RPM only
            const recent = this.getRecentRequests(provider, 60000);
            return recent < config.rpmLimit * 0.9;
        }
        
        return true;  // No limits defined
    }

    getProviderUsage(provider) {
        const config = PROVIDERS[provider];
        if (!config) return null;
        
        const available = this.isProviderAvailable(provider);
        const unavailableReason = this.limits[provider]?.unavailable;
        
        const result = {
            name: config.name,
            enabled: config.enabled,
            available: available,
            unavailableReason: unavailableReason || null,
            auth: config.auth || 'api_key',
            rateLimited: this.isRateLimited(provider),
            waitTime: this.getWaitTime(provider)
        };
        
        if (config.promptsPerWindow) {
            const windowMs = config.windowMs || (5 * 60 * 60 * 1000);
            result.used = this.getRecentRequests(provider, windowMs);
            result.limit = config.promptsPerWindow;
            result.windowHours = windowMs / (60 * 60 * 1000);
        } else if (config.rpdLimit) {
            result.used = this.getRecentDaysRequests(provider);
            result.limit = config.rpdLimit;
            result.windowHours = 24;
            if (config.rpmLimit) {
                result.rpmUsed = this.getRecentRequests(provider, 60000);
                result.rpmLimit = config.rpmLimit;
            }
        } else if (config.rpmLimit) {
            result.used = this.getRecentRequests(provider, 60000);
            result.limit = config.rpmLimit;
            result.windowHours = 1/60;  // 1 minute
        }
        
        return result;
    }
}

class PromptQueueManager extends EventEmitter {
    constructor() {
        super();
        this.queue = this.loadQueue();
        this.rateTracker = new RateLimitTracker();
        this.processing = false;
        this.currentProvider = null;
    }

    loadQueue() {
        if (!existsSync(QUEUE_FILE)) {
            return { pending: [], completed: [], failed: [] };
        }
        try {
            return JSON.parse(readFileSync(QUEUE_FILE, 'utf-8'));
        } catch {
            return { pending: [], completed: [], failed: [] };
        }
    }

    saveQueue() {
        writeFileSync(QUEUE_FILE, JSON.stringify(this.queue, null, 2));
    }

    // Add prompt to queue
    enqueue(prompt, options = {}) {
        const item = {
            id: `prompt_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
            prompt,
            options,
            priority: options.priority || 5,
            createdAt: new Date().toISOString(),
            attempts: 0,
            maxAttempts: options.maxAttempts || 3,
            provider: options.provider || null,
            status: 'pending'
        };

        // Insert by priority (lower number = higher priority)
        const insertIndex = this.queue.pending.findIndex(p => p.priority > item.priority);
        if (insertIndex === -1) {
            this.queue.pending.push(item);
        } else {
            this.queue.pending.splice(insertIndex, 0, item);
        }

        this.saveQueue();
        this.emit('enqueued', item);
        
        console.log(`📥 Enqueued: ${item.id} (priority ${item.priority})`);
        
        return item.id;
    }

    // Get next available provider
    getNextProvider() {
        const available = Object.entries(PROVIDERS)
            .filter(([key, config]) => {
                if (!config.enabled) return false;
                if (this.rateTracker.isRateLimited(key)) return false;
                return this.rateTracker.canMakeRequest(key);
            })
            .sort((a, b) => a[1].priority - b[1].priority);

        return available[0]?.[0] || null;
    }

    // Get wait time for next available provider
    getNextAvailableTime() {
        const providers = Object.keys(PROVIDERS).filter(p => PROVIDERS[p].enabled);
        
        let minWait = Infinity;
        for (const provider of providers) {
            if (!this.rateTracker.isRateLimited(provider)) {
                return 0;
            }
            const wait = this.rateTracker.getWaitTime(provider);
            minWait = Math.min(minWait, wait);
        }
        
        return minWait === Infinity ? 60000 : minWait;
    }

    // Process queue
    async processQueue(handler) {
        if (this.processing) {
            console.log('⏳ Queue already processing');
            return;
        }

        this.processing = true;
        this.emit('processing:start');

        try {
            while (this.queue.pending.length > 0) {
                const provider = this.getNextProvider();
                
                if (!provider) {
                    const waitTime = this.getNextAvailableTime();
                    console.log(`⏸️ All providers rate limited. Waiting ${Math.round(waitTime/1000)}s...`);
                    this.emit('waiting', { waitTime });
                    
                    await new Promise(r => setTimeout(r, Math.min(waitTime, 30000)));
                    continue;
                }

                const item = this.queue.pending[0];
                item.attempts++;
                item.provider = provider;
                item.status = 'processing';
                
                console.log(`\n📤 Processing: ${item.id} via ${provider}`);
                this.emit('processing', { item, provider });

                try {
                    this.rateTracker.recordRequest(provider);
                    
                    const result = await handler(item.prompt, provider, item.options);
                    
                    // Success
                    item.status = 'completed';
                    item.completedAt = new Date().toISOString();
                    item.result = result;
                    
                    this.queue.pending.shift();
                    this.queue.completed.push(item);
                    this.saveQueue();
                    
                    console.log(`✅ Completed: ${item.id}`);
                    this.emit('completed', { item, result });
                    
                } catch (error) {
                    console.log(`❌ Failed: ${item.id} - ${error.message}`);
                    
                    // Check for rate limit
                    if (error.status === 429 || error.message?.includes('rate limit')) {
                        const retryAfter = error.headers?.['retry-after'] || 60;
                        this.rateTracker.recordRateLimit(provider, parseInt(retryAfter));
                        
                        item.status = 'rate_limited';
                        this.emit('rate_limited', { item, provider, retryAfter });
                        
                        // Re-queue at front with same priority
                        continue;
                    }
                    
                    // Other error
                    item.lastError = error.message;
                    
                    if (item.attempts >= item.maxAttempts) {
                        item.status = 'failed';
                        this.queue.pending.shift();
                        this.queue.failed.push(item);
                        this.emit('failed', { item, error });
                    }
                    
                    this.saveQueue();
                }

                // Small delay between requests
                await new Promise(r => setTimeout(r, 500));
            }
        } finally {
            this.processing = false;
            this.emit('processing:end');
        }
    }

    // Get queue status
    getStatus() {
        return {
            pending: this.queue.pending.length,
            completed: this.queue.completed.length,
            failed: this.queue.failed.length,
            processing: this.processing,
            providers: Object.fromEntries(
                Object.keys(PROVIDERS).map(p => [
                    p, 
                    this.rateTracker.getProviderUsage(p)
                ])
            )
        };
    }

    // Clear completed/failed/pending items
    clear(what = 'all') {
        if (what === 'all' || what === 'completed') {
            this.queue.completed = [];
        }
        if (what === 'all' || what === 'failed') {
            this.queue.failed = [];
        }
        if (what === 'all' || what === 'pending') {
            this.queue.pending = [];
        }
        this.saveQueue();
    }

    // Retry failed items
    retryFailed() {
        const failed = this.queue.failed.splice(0);
        for (const item of failed) {
            item.status = 'pending';
            item.attempts = 0;
            item.lastError = null;
            this.queue.pending.push(item);
        }
        this.saveQueue();
        console.log(`🔄 Retrying ${failed.length} failed items`);
    }
}

// Provider-specific handlers

async function handleGLM(prompt, options = {}) {
    const apiKey = process.env.ZAI_API_KEY;
    if (!apiKey) throw new Error('ZAI_API_KEY not set');

    const config = PROVIDERS.glm;
    // Z.ai uses OpenAI-compatible chat completions endpoint
    const endpoint = `${config.baseUrl}/chat/completions`;

    const response = await fetch(endpoint, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${apiKey}`
        },
        body: JSON.stringify({
            model: options.model || config.model,
            messages: [{ role: 'user', content: prompt }],
            max_tokens: options.maxTokens || config.outputTokens
        })
    });

    if (!response.ok) {
        const body = await response.text();
        let errorMsg = `GLM error: ${response.statusText}`;
        
        try {
            const json = JSON.parse(body);
            // Handle specific error codes
            if (json.error?.code === '1113') {
                errorMsg = 'GLM: Insufficient balance or expired API key';
            } else if (json.error?.message) {
                errorMsg = `GLM: ${json.error.message}`;
            }
        } catch {}
        
        const error = new Error(errorMsg);
        error.status = response.status;
        error.headers = Object.fromEntries(response.headers);
        throw error;
    }

    const data = await response.json();
    return data.choices[0].message.content;
}

async function handleGemini(prompt, options = {}) {
    // Gemini uses OAuth - must be called via CLI tool
    const { execSync } = await import('child_process');
    
    try {
        // Try using gemini CLI if available
        const result = execSync(`gemini "${prompt.replace(/"/g, '\\"')}"`, {
            encoding: 'utf-8',
            timeout: 120000,
            maxBuffer: 10 * 1024 * 1024
        });
        return result.trim();
    } catch (error) {
        if (error.code === 'ENOENT') {
            throw new Error('Gemini CLI not found. Install with: npm install -g @anthropic-ai/gemini-cli');
        }
        // Check for auth error
        if (error.message?.includes('auth') || error.message?.includes('login') || error.status === 401) {
            console.log('\n🔑 Gemini requires OAuth login.');
            console.log('   Run: gemini auth login');
            console.log('   Or visit the URL shown in the CLI\n');
            throw new Error('Gemini: Authentication required. Run `gemini auth login`');
        }
        throw error;
    }
}

async function handleClaude(prompt, options = {}) {
    // Claude uses OAuth - must be called via Claude CLI
    const { execSync } = await import('child_process');
    
    try {
        // Try using claude CLI if available
        const result = execSync(`claude "${prompt.replace(/"/g, '\\"')}"`, {
            encoding: 'utf-8',
            timeout: 300000,  // 5 min timeout
            maxBuffer: 10 * 1024 * 1024
        });
        return result.trim();
    } catch (error) {
        if (error.code === 'ENOENT') {
            throw new Error('Claude CLI not found. Install with: npm install -g @anthropic-ai/claude-code');
        }
        // Check for auth error
        if (error.message?.includes('auth') || error.message?.includes('login') || error.status === 401) {
            console.log('\n🔑 Claude requires OAuth login.');
            console.log('   Run: claude auth login');
            console.log('   This will open a browser for you to authenticate\n');
            throw new Error('Claude: Authentication required. Run `claude auth login`');
        }
        throw error;
    }
}

async function handleLocal(prompt, options = {}) {
    const endpoint = options.endpoint || PROVIDERS.local.endpoint;
    
    // LM Studio uses OpenAI-compatible API
    const response = await fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            model: options.model || PROVIDERS.local.model,
            messages: [{ role: 'user', content: prompt }],
            max_tokens: options.maxTokens || 4096,
            temperature: options.temperature || 0.7
        })
    });

    if (!response.ok) {
        throw new Error(`Local LM Studio error: ${response.statusText}`);
    }

    const data = await response.json();
    return data.choices[0].message.content;
}

// Main handler router
async function routePrompt(prompt, provider, options = {}) {
    switch (provider) {
        case 'glm':
            return handleGLM(prompt, options);
        case 'gemini':
            return handleGemini(prompt, options);
        case 'claude':
            return handleClaude(prompt, options);
        case 'local':
            return handleLocal(prompt, options);
        default:
            throw new Error(`Unknown provider: ${provider}`);
    }
}

// CLI interface
if (import.meta.url === `file://${process.argv[1]}`) {
    const args = process.argv.slice(2);
    const cmd = args[0];
    
    const manager = new PromptQueueManager();
    
    async function checkProviders() {
        console.log('\n🔍 Checking provider credentials...\n');
        
        const home = process.env.HOME || process.env.USERPROFILE;
        
        for (const [key, config] of Object.entries(PROVIDERS)) {
            let status = '⚪';
            let note = '';
            
            if (config.auth === 'oauth') {
                // Check if CLI is available AND logged in
                try {
                    const { execSync } = await import('child_process');
                    const cliCmd = key === 'gemini' ? 'gemini --version' : 'claude --version';
                    execSync(cliCmd, { timeout: 5000, stdio: 'pipe' });
                    
                    // CLI found, now check if logged in
                    let loggedIn = false;
                    if (key === 'claude') {
                        const credPath = join(home, '.claude', 'credentials.json');
                        loggedIn = existsSync(credPath);
                    } else if (key === 'gemini') {
                        const credPath = join(home, '.config', 'gemini', 'credentials.json');
                        loggedIn = existsSync(credPath);
                    }
                    
                    if (loggedIn) {
                        status = '🟢';
                        note = '(logged in)';
                    } else {
                        status = '🟡';
                        note = '(not logged in - run: ./queue.sh login ' + key + ')';
                    }
                } catch {
                    status = '🔴';
                    note = '(CLI not installed)';
                }
            } else if (config.auth === 'none') {
                // Check if local server is running (LM Studio)
                try {
                    // LM Studio uses OpenAI-compatible endpoint
                    const checkUrl = config.endpoint.replace('/chat/completions', '/models');
                    const controller = new AbortController();
                    const timeout = setTimeout(() => controller.abort(), 2000);
                    const resp = await fetch(checkUrl, { method: 'GET', signal: controller.signal });
                    clearTimeout(timeout);
                    
                    if (resp.ok) {
                        const data = await resp.json();
                        const model = data.data?.[0]?.id || 'unknown';
                        status = '🟢';
                        note = `(server running, model: ${model})`;
                    } else {
                        status = '🔴';
                        note = '(server not running)';
                    }
                } catch {
                    status = '🔴';
                    note = '(server not running)';
                }
            } else {
                // API key auth
                const envKey = process.env[config.envKey];
                if (envKey) {
                    status = '🟢';
                    note = '(key set)';
                } else {
                    status = '🔴';
                    note = `(set ${config.envKey})`;
                }
            }
            
            console.log(`  ${status} ${key.padEnd(8)} - ${config.name} ${note}`);
        }
        console.log('');
    }
    
    (async () => {
        switch (cmd) {
            case 'status':
                if (args[1] === '--json') {
                    console.log(JSON.stringify(manager.getStatus(), null, 2));
                } else {
                    const status = manager.getStatus();
                    console.log('\n' + '═'.repeat(55));
                    console.log('  🐍 OUROBOROS QUEUE STATUS');
                    console.log('═'.repeat(55));
                    
                    console.log('\n  📊 Queue:');
                    console.log(`     Pending:   ${status.pending}`);
                    console.log(`     Completed: ${status.completed}`);
                    console.log(`     Failed:    ${status.failed}`);
                    
                    console.log('\n  🔌 Providers:');
                    for (const [key, info] of Object.entries(status.providers)) {
                        if (!info) continue;
                        
                        // Determine icon based on availability
                        let icon = '⚪';
                        if (!info.enabled) {
                            icon = '⚪';
                        } else if (!info.available) {
                            icon = '🔴';
                        } else if (info.rateLimited) {
                            icon = '🟡';
                        } else {
                            icon = '🟢';
                        }
                        
                        const authIcon = info.auth === 'oauth' ? '🔑' : (info.auth === 'none' ? '🔓' : '');
                        const pct = info.limit ? Math.round((info.used / info.limit) * 100) : 0;
                        const barLen = 20;
                        const filled = Math.round((pct / 100) * barLen);
                        const bar = '█'.repeat(filled) + '░'.repeat(barLen - filled);
                        
                        let usage = '';
                        if (!info.available) {
                            usage = `⚠️ ${info.unavailableReason}`;
                        } else if (info.windowHours === 24) {
                            usage = `${info.used}/${info.limit}/day (${pct}%)`;
                            if (info.rpmLimit) {
                                usage += ` [${info.rpmUsed}/${info.rpmLimit}/min]`;
                            }
                        } else if (info.windowHours < 1) {
                            usage = `${info.used}/${info.limit}/min (${pct}%)`;
                        } else {
                            usage = `${info.used}/${info.limit}/${info.windowHours}hr (${pct}%)`;
                        }
                        
                        console.log(`     ${icon} ${key.padEnd(8)} [${bar}] ${usage} ${authIcon}`);
                        if (info.rateLimited && info.waitTime > 0) {
                            console.log(`              ⏸️ Rate limited for ${Math.round(info.waitTime/1000)}s`);
                        }
                    }
                    console.log('\n  🟢 = Available  🟡 = Rate limited  🔴 = Unavailable  ⚪ = Disabled');
                    console.log('  🔑 = OAuth (CLI)  🔓 = No auth');
                    console.log('═'.repeat(55) + '\n');
                }
                break;
                
            case 'enqueue':
                const prompt = args.slice(1).join(' ');
                if (!prompt) {
                    console.log('Usage: node prompt_queue.js enqueue "your prompt"');
                    process.exit(1);
                }
                manager.enqueue(prompt);
                break;
                
            case 'process':
                await manager.processQueue(routePrompt);
                console.log('\n📊 Final status:', manager.getStatus());
                break;
                
            case 'clear':
                manager.clear(args[1] || 'all');
                console.log('Cleared:', args[1] || 'all');
                break;
                
            case 'retry':
                manager.retryFailed();
                break;
                
            case 'check':
                await checkProviders();
                break;
                
            case 'login':
                // Help user login to OAuth providers
                const provider = args[1];
                if (!provider || !['gemini', 'claude'].includes(provider)) {
                    console.log('\nUsage: queue.sh login <gemini|claude>\n');
                    console.log('This will open a browser for OAuth authentication.\n');
                    break;
                }
                
                console.log(`\n🔑 Starting ${provider} OAuth login...\n`);
                
                try {
                    const { spawn } = await import('child_process');
                    const loginCmd = provider === 'gemini' ? 'gemini' : 'claude';
                    
                    // Run the auth login command interactively
                    const child = spawn(loginCmd, ['auth', 'login'], {
                        stdio: 'inherit',
                        shell: true
                    });
                    
                    child.on('exit', (code) => {
                        if (code === 0) {
                            console.log(`\n✅ ${provider} login successful!\n`);
                        } else {
                            console.log(`\n❌ ${provider} login failed or cancelled.\n`);
                        }
                    });
                } catch (error) {
                    console.log(`\n❌ Failed to start login: ${error.message}\n`);
                }
                break;
                
            case 'enable':
            case 'disable':
                const toggleProvider = args[1];
                if (!toggleProvider || !PROVIDERS[toggleProvider]) {
                    console.log(`\nUsage: queue.sh ${cmd} <provider>\n`);
                    console.log('Providers: ' + Object.keys(PROVIDERS).join(', ') + '\n');
                    break;
                }
                PROVIDERS[toggleProvider].enabled = (cmd === 'enable');
                console.log(`\n${cmd === 'enable' ? '✅' : '🔴'} ${toggleProvider} ${cmd}d\n`);
                break;
                
            default:
                console.log(`
Prompt Queue Manager - Multi-Provider Rate Limit Handler

Commands:
  status [--json]  Show queue status and provider usage
  dashboard [--watch]  Show visual dashboard (live with --watch)
  enqueue "..."    Add prompt to queue
  process          Process queued prompts
  clear [type]     Clear completed/failed/pending/all items
  retry            Retry all failed items
  check            Check provider credentials
  login <provider> Login to OAuth provider (gemini|claude)
  enable <provider>  Enable a provider
  disable <provider> Disable a provider

Environment Variables:
  ZAI_API_KEY         - For GLM Coding Max (API key auth)
  
Note: Gemini and Claude use OAuth - must use their CLI tools:
  gemini - run 'gemini auth login' to authenticate
  claude - run 'claude auth login' to authenticate

Local: LM Studio must be running on localhost:1234

Providers (in priority order):
  1. glm    - GLM Coding Max (2400 prompts/5hr) - $216/qtr
  2. gemini - Gemini Code Assist (1500/day, 120/min) - $19/mo
  3. claude - Claude Pro (10-40 prompts/5hr) - $20/mo
  4. local  - LM Studio (unlimited) - free, requires local server

Queue file: ${QUEUE_FILE}
`);
        }
    })().catch(console.error);
}

export { PromptQueueManager, RateLimitTracker, routePrompt, PROVIDERS };
