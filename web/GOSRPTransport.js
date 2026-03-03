/**
 * GOS-RP WebRTC Transport Layer
 * 
 * Enables real peer-to-peer visual packet exchange using WebRTC DataChannels.
 */

export class GOSRPTransport {
    constructor(router) {
        this.router = router;
        this.connections = new Map(); // Peer Address -> RTCPeerConnection
        this.dataChannels = new Map(); // Peer Address -> RTCDataChannel
    }

    /**
     * Create a connection offer for a peer
     */
    async createOffer(peerAddr) {
        const pc = new RTCPeerConnection({
            iceServers: [{ urls: 'stun:stun.l.google.com:19302' }]
        });
        
        const dc = pc.createDataChannel('gos-rp-control');
        this._setupDataChannel(peerAddr, dc);
        
        const offer = await pc.createOffer();
        await pc.setLocalDescription(offer);
        
        this.connections.set(peerAddr, pc);
        console.log(`[GOS Transport] Created offer for 0x${peerAddr.toString(16)}`);
        
        return offer;
    }

    /**
     * Handle an incoming offer and create an answer
     */
    async handleOffer(peerAddr, offer) {
        const pc = new RTCPeerConnection({
            iceServers: [{ urls: 'stun:stun.l.google.com:19302' }]
        });
        
        pc.ondatachannel = (event) => {
            this._setupDataChannel(peerAddr, event.channel);
        };
        
        await pc.setRemoteDescription(new RTCSessionDescription(offer));
        const answer = await pc.createAnswer();
        await pc.setLocalDescription(answer);
        
        this.connections.set(peerAddr, pc);
        return answer;
    }

    _setupDataChannel(peerAddr, dc) {
        dc.onopen = () => {
            console.log(`[GOS Transport] DataChannel open with 0x${peerAddr.toString(16)}`);
            this.dataChannels.set(peerAddr, dc);
            
            // Send initial Ping
            this.sendPacket(peerAddr, { type: 'PING' });
        };
        
        dc.onmessage = (event) => {
            const msg = JSON.parse(event.data);
            this._handleIncomingPacket(peerAddr, msg);
        };
        
        dc.onclose = () => {
            this.dataChannels.delete(peerAddr);
            this.connections.delete(peerAddr);
        };
    }

    /**
     * Send a visual packet or control message to a peer
     */
    sendPacket(peerAddr, packet) {
        const dc = this.dataChannels.get(peerAddr);
        if (dc && dc.readyState === 'open') {
            dc.send(JSON.stringify({
                ...packet,
                sender: this.router.myAddress,
                timestamp: Date.now()
            }));
        }
    }

    _handleIncomingPacket(peerAddr, packet) {
        console.log(`[GOS Transport] Received packet from 0x${peerAddr.toString(16)}:`, packet.type);
        
        if (packet.type === 'ROUTE_AD') {
            this.router.receiveAdvertisement(packet.target, packet.distance, packet.sender);
        } else if (packet.type === 'PING') {
            this.sendPacket(peerAddr, { type: 'PONG' });
        }
    }

    /**
     * Broadcast a packet to all connected peers
     */
    broadcast(packet) {
        for (const [peerAddr, dc] of this.dataChannels) {
            this.sendPacket(peerAddr, packet);
        }
    }
}
