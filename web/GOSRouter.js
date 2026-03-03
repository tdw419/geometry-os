/**
 * Geometry OS Router
 *
 * GPU-side packet routing for inter-process communication.
 * Implements a TCP/IP-like stack on WebGPU.
 */

// Packet types
const PACKET_TYPES = {
    DATA: 0x01,
    ACK: 0x02,
    NACK: 0x03,
    SYN: 0x04,
    SYN_ACK: 0x05,
    FIN: 0x06,
    RST: 0x07,
    HEARTBEAT: 0x08,
    ROUTE: 0x09,
    FRAGMENT: 0x0A
};

const PORTS = {
    IPC: 5000,
    FS: 5001,
    KERNEL: 5002,
    SHELL: 5003,
    DISPLAY: 5004,
    AUDIO: 5005,
    NET: 5006
};
const PACKET_FLAGS = {
    NONE: 0x00,
    URGENT: 0x01,
    BROADCAST: 0x02,
    ENCRYPTED: 0x04,
    COMPRESSED: 0x08
};
const MAX_PACKET_SIZE = 1024;
const MAX_QUEUE_SIZE = 256;
const ACK_TIMEOUT = 1000;
const SYN_TIMEOUT = 500;

class PacketHeader {
    constructor() {
        this.srcPort = 0;
        this.dstPort = 0;
        this.type = 0;
        this.flags = 0;
        this.seqNum = 0;
        this.ackNum = 0;
        this.length = 0;
    }
    encode() {
        return new Uint8Array([
            this.srcPort >> 8, this.srcPort & 0xFF,
            this.dstPort >> 8, this.dstPort & 0xFF,
            this.type, this.flags,
            this.seqNum >> 24, (this.seqNum >> 16) & 0xFF, this.seqNum & 0xFF,
            this.ackNum >> 24, (this.ackNum >> 16) & 0xFF, this.ackNum & 0xFF,
            this.length >> 8, this.length & 0xFF
        ]);
    }
    static decode(data) {
        const h = new PacketHeader();
        h.srcPort = (data[0] << 8) | data[1];
        h.dstPort = (data[2] << 8) | data[3];
        h.type = data[4];
        h.flags = data[5];
        h.seqNum = (data[6] << 24) | (data[7] << 16) | data[8];
        h.ackNum = (data[9] << 24) | (data[10] << 16) | data[11];
        h.length = (data[12] << 8) | data[13];
        return h;
    }
}

class NetworkPacket {
    constructor(header, payload = null) {
        this.header = header;
        this.payload = payload || new Uint8Array(0);
        this.timestamp = Date.now();
        this.retries = 0;
    }
    get size() { return 16 + this.payload.length; }
    serialize() {
        const headerBytes = this.header.encode();
        const total = new Uint8Array(16 + this.payload.length);
        total.set(headerBytes);
        total.set(this.payload, 16);
        return total;
    }
}

class PacketQueue {
    constructor(port, maxSize = MAX_QUEUE_SIZE) {
        this.port = port;
        this.maxSize = maxSize;
        this.queue = [];
        this.nextSeq = 1;
        this.pending = new Map();
    }
    enqueue(packet) {
        if (this.queue.length >= this.maxSize) return false;
        this.queue.push(packet);
        return true;
    }
    dequeue() {
        return this.queue.length > 0 ? this.queue.shift() : null;
    }
    peek() { return this.queue.length > 0 ? this.queue[0] : null; }
    get size() { return this.queue.length; }
    isEmpty() { return this.queue.length === 0; }
    isFull() { return this.queue.length >= this.maxSize; }
    clear() { this.queue = []; this.pending.clear(); }
}

class ReliableTransport {
    constructor(router) {
        this.router = router;
        this.unacked = new Map();
        this.windowSize = 16;
        this.timeout = ACK_TIMEOUT;
    }
    send(packet) {
        if (this.unacked.size >= this.windowSize) this._timeoutOldest();
        this.unacked.set(packet.header.seqNum, {
            packet, sendTime: Date.now(), retries: 0
        });
        this.router.route(packet);
    }
    ack(srcPort, dstPort, seqNum) {
        this.unacked.delete(seqNum);
    }
    nack(srcPort, dstPort, seqNum) {
        const entry = this.unacked.get(seqNum);
        if (entry) {
            entry.retries++;
            if (entry.retries > 3) {
                this.unacked.delete(seqNum);
            } else {
                entry.sendTime = Date.now();
                this.router.route(entry.packet);
            }
        }
    }
    checkTimeouts() {
        const now = Date.now();
        for (const [seqNum, entry] of this.unacked) {
            if (now - entry.sendTime > this.timeout) {
                this.nack(entry.packet.header.dstPort, entry.packet.header.srcPort, seqNum);
                this.unacked.delete(seqNum);
            }
        }
    }
    _timeoutOldest() {
        let oldest = null, oldestTime = Infinity;
        for (const [seqNum, entry] of this.unacked) {
            if (entry.sendTime < oldestTime) {
                oldestTime = entry.sendTime;
                oldest = seqNum;
            }
        }
        if (oldest) this.unacked.delete(oldest);
    }
}

export class GOSRouter {
    constructor() {
        this.queues = new Map();
        for (const [name, port] of Object.entries(PORTS)) {
            this.queues.set(port, new PacketQueue(port));
        }
        this.transport = new ReliableTransport(this);
        this.handlers = new Map();
        this.stats = { packetsSent: 0, packetsReceived: 0, packetsDropped: 0, bytesTransferred: 0 };
    }
    on(port, handler) { this.handlers.set(port, handler); }
    off(port) { this.handlers.delete(port); }
    send(srcPort, dstPort, type, payload, flags = 0) {
        const header = new PacketHeader();
        header.srcPort = srcPort;
        header.dstPort = dstPort;
        header.type = type;
        header.flags = flags;
        header.seqNum = this._nextSeq(srcPort);
        header.ackNum = 0;
        header.length = payload.length;
        const packet = new NetworkPacket(header, payload);
        this.stats.packetsSent++;
        this.stats.bytesTransferred += packet.size;
        this.transport.send(packet);
    }
    route(packet) {
        const dstPort = packet.header.dstPort;
        const queue = this.queues.get(dstPort);
        if (!queue) { this.stats.packetsDropped++; return false; }
        if (!queue.enqueue(packet)) { this.stats.packetsDropped++; return false; }
        const handler = this.handlers.get(dstPort);
        if (handler) {
            this.stats.packetsReceived++;
            handler(packet);
        }
        return true;
    }
    receive(port) {
        const queue = this.queues.get(port);
        if (!queue) return null;
        return queue.dequeue();
    }
    peek(port) {
        const queue = this.queues.get(port);
        if (!queue) return null;
        return queue.peek();
    }
    broadcast(srcPort, type, payload, excludeSrc = true) {
        for (const port of this.queues.keys()) {
            if (excludeSrc && port === srcPort) continue;
            this.send(srcPort, port, type, payload, PACKET_FLAGS.BROADCAST);
        }
    }
    ack(srcPort, dstPort, seqNum) { this.transport.ack(srcPort, dstPort, seqNum); }
    nack(srcPort, dstPort, seqNum) { this.transport.nack(srcPort, dstPort, seqNum); }
    _nextSeq(port) {
        const queue = this.queues.get(port);
        if (!queue) return 1;
        return queue.nextSeq++;
    }
    getStats() {
        const queueStats = {};
        for (const [port, queue] of this.queues) {
            queueStats[port] = { size: queue.size, maxSize: queue.maxSize };
        }
        return { ...this.stats, queues: queueStats, unacked: this.transport.unacked.size };
    }
}
