/**
 * Geometry OS Interaction Manager
 *
 * Enables bi-directional haptic manipulation of the substrate.
 * Users can physically grab and move memory particles, files, and agent avatars.
 *
 * "Moving a pixel IS moving a file."
 *
 * Features:
 * - Grab/drag any entity in 3D space
 * - Back-propagation: Visual moves update database
 * - Agent repulsion/attraction physics
 * - Semantic gravity based on proximity
 */

// Interaction modes
const INTERACTION_MODES = {
    SELECT: 0,
    GRAB: 1,
    SCALE: 2,
    ROTATE: 3,
    TETHER: 4
};

// Entity types that can be manipulated
const ENTITY_TYPES = {
    MEMORY: 'memory',
    FILE: 'file',
    AGENT: 'agent',
    WINDOW: 'window',
    PROCESS: 'process'
};

/**
 * Represents a grabbed entity during manipulation
 */
class GrabbedEntity {
    constructor(options) {
        this.id = options.id;
        this.type = options.type;
        this.originalPosition = { ...options.position };
        this.currentPosition = { ...options.position };
        this.entity = options.entity;

        // Physics
        this.velocity = { x: 0, y: 0, z: 0 };
        this.mass = options.mass || 1.0;

        // State
        this.modified = false;
        this.grabTime = Date.now();
    }

    /**
     * Update position with physics
     */
    update(targetPos, dt) {
        const dx = targetPos.x - this.currentPosition.x;
        const dy = targetPos.y - this.currentPosition.y;
        const dz = targetPos.z - this.currentPosition.z;

        // Spring physics
        const k = 0.3;  // Spring constant
        const damping = 0.8;

        this.velocity.x = (this.velocity.x + dx * k) * damping;
        this.velocity.y = (this.velocity.y + dy * k) * damping;
        this.velocity.z = (this.velocity.z + dz * k) * damping;

        this.currentPosition.x += this.velocity.x;
        this.currentPosition.y += this.velocity.y;
        this.currentPosition.z += this.velocity.z;

        // Check if modified from original
        const dist = Math.sqrt(
            Math.pow(this.currentPosition.x - this.originalPosition.x, 2) +
            Math.pow(this.currentPosition.y - this.originalPosition.y, 2)
        );
        this.modified = dist > 5;
    }

    /**
     * Commit the position change
     */
    commit() {
        return {
            id: this.id,
            type: this.type,
            oldPosition: this.originalPosition,
            newPosition: { ...this.currentPosition }
        };
    }
}

/**
 * InteractionManager - Handles all haptic interactions
 */
export class InteractionManager {
    constructor(os) {
        this.os = os;
        this.device = os.device;
        this.canvas = os.canvas;

        // Interaction state
        this.mode = INTERACTION_MODES.SELECT;
        this.grabbed = null;
        this.hovered = null;
        this.selected = null;

        // Ray picking
        this.rayOrigin = { x: 0, y: 0, z: 0 };
        this.rayDirection = { x: 0, y: 0, z: -1 };
        this.pickResult = null;

        // Entity registries (populated from managers)
        this.memories = new Map();
        this.files = new Map();
        this.agents = new Map();
        this.processes = new Map();

        // Physics
        this.semanticGravity = 0.01;
        this.agentAttraction = 0.005;
        this.repulsionRadius = 50;

        // Gesture state
        this.gestureStart = null;
        this.lastMousePos = { x: 0, y: 0 };
        this.isDragging = false;

        // Undo stack
        this.undoStack = [];
        this.maxUndoSteps = 50;

        // Callbacks
        this.onEntityMoved = null;
        this.onEntitySelected = null;
        this.onAgentFocused = null;
    }

    /**
     * Initialize the interaction manager
     */
    async init() {
        this._wireEvents();
        this._initPickBuffer();

        console.log('[InteractionManager] Haptic interactions enabled');
        return this;
    }

    _initPickBuffer() {
        // GPU buffer for picking results
        this.pickBuffer = this.device.createBuffer({
            size: 16,  // 4 floats: entity ID, type, depth, padding
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });
    }

    _wireEvents() {
        // Mouse events
        this.canvas.addEventListener('mousedown', (e) => this._onMouseDown(e));
        this.canvas.addEventListener('mousemove', (e) => this._onMouseMove(e));
        this.canvas.addEventListener('mouseup', (e) => this._onMouseUp(e));
        this.canvas.addEventListener('dblclick', (e) => this._onDoubleClick(e));

        // Touch events
        this.canvas.addEventListener('touchstart', (e) => this._onTouchStart(e));
        this.canvas.addEventListener('touchmove', (e) => this._onTouchMove(e));
        this.canvas.addEventListener('touchend', (e) => this._onTouchEnd(e));

        // Keyboard
        window.addEventListener('keydown', (e) => this._onKeyDown(e));
        window.addEventListener('keyup', (e) => this._onKeyUp(e));

        // Wheel for zoom
        this.canvas.addEventListener('wheel', (e) => this._onWheel(e));
    }

    /**
     * Register entities from various managers
     */
    registerMemories(memories) {
        for (const memory of memories) {
            this.memories.set(memory.id, {
                id: memory.id,
                type: ENTITY_TYPES.MEMORY,
                position: memory.position || { x: 0, y: 0, z: 0 },
                entity: memory
            });
        }
    }

    registerFiles(files) {
        for (const file of files) {
            this.files.set(file.path, {
                id: file.path,
                type: ENTITY_TYPES.FILE,
                position: file.position || { x: 0, y: 0, z: 0 },
                entity: file
            });
        }
    }

    registerAgents(agents) {
        for (const agent of agents) {
            this.agents.set(agent.id, {
                id: agent.id,
                type: ENTITY_TYPES.AGENT,
                position: agent.position || { x: 0, y: 0, z: 0 },
                entity: agent,
                mass: 0.5  // Agents are lighter
            });
        }
    }

    registerProcesses(processes) {
        for (const proc of processes) {
            this.processes.set(proc.pid, {
                id: proc.pid,
                type: ENTITY_TYPES.PROCESS,
                position: proc.position || { x: 0, y: 0, z: 0 },
                entity: proc
            });
        }
    }

    /**
     * Convert screen coordinates to world ray
     */
    screenToWorldRay(screenX, screenY) {
        const rect = this.canvas.getBoundingClientRect();
        const ndcX = (screenX - rect.left) / rect.width * 2 - 1;
        const ndcY = -((screenY - rect.top) / rect.height * 2 - 1);

        // Simple orthographic unprojection
        const cam = this.os.cameraPosition;
        const zoom = cam.z * 0.01;

        return {
            origin: {
                x: cam.x + ndcX * this.canvas.width * zoom * 0.5,
                y: cam.y + ndcY * this.canvas.height * zoom * 0.5,
                z: cam.z
            },
            direction: { x: 0, y: 0, z: -1 }
        };
    }

    /**
     * Ray-box intersection test
     */
    rayBoxIntersect(ray, box) {
        const EPSILON = 0.0001;

        let tmin = (box.min.x - ray.origin.x) / (ray.direction.x || EPSILON);
        let tmax = (box.max.x - ray.origin.x) / (ray.direction.x || EPSILON);

        if (tmin > tmax) [tmin, tmax] = [tmax, tmin];

        let tymin = (box.min.y - ray.origin.y) / (ray.direction.y || EPSILON);
        let tymax = (box.max.y - ray.origin.y) / (ray.direction.y || EPSILON);

        if (tymin > tymax) [tymin, tymax] = [tymax, tymin];

        if (tmin > tymax || tymin > tmax) return null;

        tmin = Math.max(tmin, tymin);
        tmax = Math.min(tmax, tymax);

        const tzmin = (box.min.z - ray.origin.z) / (ray.direction.z || EPSILON);
        const tzmax = (box.max.z - ray.origin.z) / (ray.direction.z || EPSILON);

        if (tzmin > tzmax) return null;

        if (tmin > tzmax || tzmin > tmax) return null;

        return {
            t: Math.max(tmin, tzmin),
            point: {
                x: ray.origin.x + ray.direction.x * tmin,
                y: ray.origin.y + ray.direction.y * tmin,
                z: ray.origin.z + ray.direction.z * tmin
            }
        };
    }

    /**
     * Pick entity at screen position
     */
    pickEntity(screenX, screenY) {
        const ray = this.screenToWorldRay(screenX, screenY);

        let closest = null;
        let closestDist = Infinity;

        // Check all entity types
        const allEntities = [
            ...this.memories.values(),
            ...this.files.values(),
            ...this.agents.values(),
            ...this.processes.values()
        ];

        for (const entity of allEntities) {
            const size = entity.type === ENTITY_TYPES.AGENT ? 20 : 15;
            const box = {
                min: {
                    x: entity.position.x - size,
                    y: entity.position.y - size,
                    z: entity.position.z - size
                },
                max: {
                    x: entity.position.x + size,
                    y: entity.position.y + size,
                    z: entity.position.z + size
                }
            };

            const hit = this.rayBoxIntersect(ray, box);
            if (hit && hit.t < closestDist) {
                closestDist = hit.t;
                closest = entity;
            }
        }

        // Also check windows
        for (const [id, win] of this.os.windows) {
            const hw = win.size.width / 2;
            const hh = win.size.height / 2;
            const box = {
                min: { x: win.position.x - hw, y: win.position.y - hh, z: win.position.z - 1 },
                max: { x: win.position.x + hw, y: win.position.y + hh, z: win.position.z + 1 }
            };

            const hit = this.rayBoxIntersect(ray, box);
            if (hit && hit.t < closestDist) {
                closestDist = hit.t;
                closest = { id, type: ENTITY_TYPES.WINDOW, position: win.position, entity: win };
            }
        }

        return closest;
    }

    _onMouseDown(e) {
        if (e.button !== 0) return;  // Left click only

        this.lastMousePos = { x: e.clientX, y: e.clientY };
        this.gestureStart = { x: e.clientX, y: e.clientY, time: Date.now() };

        const picked = this.pickEntity(e.clientX, e.clientY);

        if (picked) {
            this.selected = picked;
            this.mode = INTERACTION_MODES.GRAB;

            this.grabbed = new GrabbedEntity({
                id: picked.id,
                type: picked.type,
                position: picked.position,
                entity: picked.entity
            });

            this.isDragging = true;

            // Emit selection event
            if (this.onEntitySelected) {
                this.onEntitySelected(picked);
            }

            console.log(`[Interaction] Grabbed ${picked.type}: ${picked.id}`);
        }
    }

    _onMouseMove(e) {
        this.lastMousePos = { x: e.clientX, y: e.clientY };

        if (this.grabbed && this.isDragging) {
            // Calculate world position under cursor
            const ray = this.screenToWorldRay(e.clientX, e.clientY);
            const targetPos = {
                x: ray.origin.x,
                y: ray.origin.y,
                z: this.grabbed.currentPosition.z
            };

            this.grabbed.update(targetPos, 16);

            // Update entity position
            if (this.grabbed.entity) {
                this.grabbed.entity.position = { ...this.grabbed.currentPosition };
            }

            // Apply agent attraction if dragging an agent
            if (this.grabbed.type === ENTITY_TYPES.AGENT) {
                this._applyAgentPhysics(this.grabbed);
            }
        } else {
            // Hover detection
            const hovered = this.pickEntity(e.clientX, e.clientY);
            if (hovered !== this.hovered) {
                this.hovered = hovered;
                this._updateCursor(hovered);
            }
        }
    }

    _onMouseUp(e) {
        if (this.grabbed) {
            const change = this.grabbed.commit();

            if (this.grabbed.modified) {
                // Push to undo stack
                this.undoStack.push({
                    type: 'move',
                    entity: this.grabbed,
                    from: change.oldPosition,
                    to: change.newPosition
                });

                if (this.undoStack.length > this.maxUndoSteps) {
                    this.undoStack.shift();
                }

                // Back-propagate to database
                this._backPropagate(this.grabbed);

                console.log(`[Interaction] Moved ${change.type} ${change.id}: (${change.oldPosition.x}, ${change.oldPosition.y}) → (${change.newPosition.x}, ${change.newPosition.y})`);
            }

            // Emit move event
            if (this.onEntityMoved && this.grabbed.modified) {
                this.onEntityMoved(change);
            }
        }

        this.grabbed = null;
        this.isDragging = false;
        this.mode = INTERACTION_MODES.SELECT;
    }

    _onDoubleClick(e) {
        const picked = this.pickEntity(e.clientX, e.clientY);

        if (picked) {
            // Focus/activate entity
            this._activateEntity(picked);
        }
    }

    _onTouchStart(e) {
        if (e.touches.length === 1) {
            const touch = e.touches[0];
            this._onMouseDown({ clientX: touch.clientX, clientY: touch.clientY, button: 0 });
        }
    }

    _onTouchMove(e) {
        if (e.touches.length === 1 && this.grabbed) {
            const touch = e.touches[0];
            this._onMouseMove({ clientX: touch.clientX, clientY: touch.clientY });
            e.preventDefault();
        }
    }

    _onTouchEnd(e) {
        this._onMouseUp({});
    }

    _onKeyDown(e) {
        // Undo
        if (e.ctrlKey && e.key === 'z') {
            this.undo();
            e.preventDefault();
        }

        // Mode switching
        if (e.key === 'g') {
            this.mode = INTERACTION_MODES.GRAB;
        } else if (e.key === 't') {
            this.mode = INTERACTION_MODES.TETHER;
        }

        // Escape to release
        if (e.key === 'Escape' && this.grabbed) {
            this._cancelGrab();
        }

        // Delete selected
        if (e.key === 'Delete' && this.selected) {
            this._deleteEntity(this.selected);
        }
    }

    _onKeyUp(e) {
        if (e.key === 'g' || e.key === 't') {
            this.mode = INTERACTION_MODES.SELECT;
        }
    }

    _onWheel(e) {
        // Zoom is handled by GeometryOS, but we can add semantic zoom here
    }

    _updateCursor(hovered) {
        if (hovered) {
            this.canvas.style.cursor = this.mode === INTERACTION_MODES.GRAB ? 'grabbing' : 'grab';
        } else {
            this.canvas.style.cursor = 'default';
        }
    }

    _applyAgentPhysics(agent) {
        // Agents are attracted to relevant sectors based on their task
        const sectorCenters = {
            [ENTITY_TYPES.MEMORY]: { x: 1000, y: 300 },
            [ENTITY_TYPES.FILE]: { x: 600, y: 300 },
            [ENTITY_TYPES.PROCESS]: { x: 200, y: 300 }
        };

        // Apply gentle attraction toward relevant sectors
        for (const [type, center] of Object.entries(sectorCenters)) {
            const dx = center.x - agent.currentPosition.x;
            const dy = center.y - agent.currentPosition.y;
            const dist = Math.sqrt(dx * dx + dy * dy);

            if (dist < 300) {
                agent.currentPosition.x += dx * this.agentAttraction;
                agent.currentPosition.y += dy * this.agentAttraction;
            }
        }

        // Repulsion from other agents
        for (const [id, other] of this.agents) {
            if (id === agent.id) continue;

            const dx = agent.currentPosition.x - other.position.x;
            const dy = agent.currentPosition.y - other.position.y;
            const dist = Math.sqrt(dx * dx + dy * dy);

            if (dist < this.repulsionRadius && dist > 0) {
                const force = (this.repulsionRadius - dist) / this.repulsionRadius * 0.1;
                agent.currentPosition.x += (dx / dist) * force;
                agent.currentPosition.y += (dy / dist) * force;
            }
        }
    }

    _cancelGrab() {
        if (this.grabbed) {
            // Restore original position
            this.grabbed.currentPosition = { ...this.grabbed.originalPosition };
            if (this.grabbed.entity) {
                this.grabbed.entity.position = { ...this.grabbed.originalPosition };
            }
            this.grabbed = null;
            this.isDragging = false;
        }
    }

    /**
     * Back-propagate visual change to database
     */
    async _backPropagate(grabbed) {
        const { id, type, currentPosition } = grabbed;

        // Different back-propagation based on entity type
        switch (type) {
            case ENTITY_TYPES.MEMORY:
                // Update memory embedding/position in PostgreSQL
                console.log(`[Interaction] Back-propagating memory ${id} to database`);
                // TODO: Call open_brain API to update memory position
                break;

            case ENTITY_TYPES.FILE:
                // Update file metadata
                console.log(`[Interaction] Back-propagating file ${id} to filesystem`);
                // TODO: Update file position in file manager
                break;

            case ENTITY_TYPES.AGENT:
                // Update agent focus
                console.log(`[Interaction] Back-propagating agent ${id} position`);
                if (this.onAgentFocused) {
                    this.onAgentFocused(grabbed.entity, currentPosition);
                }
                break;

            case ENTITY_TYPES.PROCESS:
                // Process moved - could affect memory isolation
                console.log(`[Interaction] Process ${id} moved to new memory region`);
                break;
        }
    }

    _activateEntity(entity) {
        switch (entity.type) {
            case ENTITY_TYPES.FILE:
                // Open file in editor
                this.os.focusWindow('files');
                break;

            case ENTITY_TYPES.AGENT:
                // Focus agent, show its thoughts
                console.log(`[Interaction] Activated agent: ${entity.id}`);
                break;

            case ENTITY_TYPES.PROCESS:
                // Focus process in shell
                this.os.focusWindow('shell');
                break;

            case ENTITY_TYPES.WINDOW:
                // Bring window to front
                this.os.focusWindow(entity.id);
                break;
        }
    }

    _deleteEntity(entity) {
        // Only allow deletion of certain types
        if (entity.type === ENTITY_TYPES.PROCESS) {
            // Kill process
            console.log(`[Interaction] Killing process ${entity.id}`);
            // TODO: Call kernel to terminate process
        }
    }

    /**
     * Undo last action
     */
    undo() {
        if (this.undoStack.length === 0) return;

        const action = this.undoStack.pop();

        if (action.type === 'move') {
            // Restore position
            action.entity.currentPosition = { ...action.from };
            if (action.entity.entity) {
                action.entity.entity.position = { ...action.from };
            }

            console.log(`[Interaction] Undid move: ${action.entity.id}`);
        }
    }

    /**
     * Create a tether between two entities
     */
    createTether(fromId, toId) {
        const from = this.memories.get(fromId) || this.files.get(fromId) || this.agents.get(fromId);
        const to = this.memories.get(toId) || this.files.get(toId) || this.agents.get(toId);

        if (from && to) {
            // Emit tether event for visualization
            this.os.emit('tether', {
                from: from.position,
                to: to.position,
                fromId,
                toId
            });

            console.log(`[Interaction] Created tether: ${fromId} → ${toId}`);
        }
    }

    /**
     * Update interaction manager (call each frame)
     */
    update(dt) {
        // Update any grabbed entity physics
        if (this.grabbed) {
            this.grabbed.update(this.grabbed.currentPosition, dt);
        }
    }

    /**
     * Get interaction stats
     */
    getStats() {
        return {
            mode: Object.keys(INTERACTION_MODES)[this.mode],
            grabbed: this.grabbed?.id || null,
            selected: this.selected?.id || null,
            hovered: this.hovered?.id || null,
            undoStackDepth: this.undoStack.length,
            entityCounts: {
                memories: this.memories.size,
                files: this.files.size,
                agents: this.agents.size,
                processes: this.processes.size
            }
        };
    }
}

// Export types
export { INTERACTION_MODES, ENTITY_TYPES, GrabbedEntity };
