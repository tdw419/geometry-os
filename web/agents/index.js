/**
 * Area Agents Module Index
 *
 * Provides the 7 Area Agents for Geometry OS kernel processes.
 */

export {
    AGENTS,
    IPC,
    PROC_STATE,
    AgentGenerator,
    generateCompositorAgent,
    generateShellAgent,
    generateCognitiveAgent,
    generateMemoryAgent,
    generateIOAgent,
    generateSchedulerAgent,
    generateNetworkAgent,
    generateAllAgents
} from './AgentGenerator.js';
