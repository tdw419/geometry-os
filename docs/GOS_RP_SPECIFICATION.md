# GOS-RP: Geometry OS Routing Protocol Specification

## 1. Overview
GOS-RP is a morphological routing protocol designed for the Geometry OS ecosystem. It treats network addresses and routing tables as spatial structures mapped via the **Hilbert Curve**. Routing decisions are influenced by **Spatial Locality**, where proximity in the 1D Hilbert sequence correlates to logical or physical network proximity.

## 2. Addressing Scheme
Addresses are 32-bit unsigned integers divided into three semantic segments:
- **Hilbert Region (8 bits):** The top-level spatial zone (0-255). Used for initial routing and discovery.
- **Network Cluster (8 bits):** Logical grouping of nodes within a region.
- **Host ID (16 bits):** Unique identifier for a node within a cluster.

`Address = (Region << 24) | (Cluster << 16) | (HostID)`

## 3. Visual Control Plane
Control messages are encoded as **Visual Packets**—8x8 grids of semantic glyphs. 

### 3.1 Packet Layout (Hilbert Ordered)
| Index | Field | Description |
|-------|-------|-------------|
| 0 | Type | 0xA0 (Route Ad), 0xB5 (Ping), 0xB6 (Pong) |
| 1-4 | Target | 32-bit Target Address (Big-endian) |
| 5 | Metric | Hop count (0-16) |
| 6-9 | Sender | 32-bit Sender Address |
| 10+ | Padding | Visual noise or cryptographic signatures |

## 4. Routing Logic
GOS-RP utilizes a **Distance-Vector** algorithm with several Geometry OS-specific enhancements.

### 4.1 Discovery (Hilbert Proximity)
Nodes discover peers by probing adjacent indices in the Hilbert sequence ($N-1$ and $N+1$). This ensures that the network graph mirrors the spatial topology of the 2D visual grid.

### 4.2 Distance-Vector Update
Upon receiving a Route Advertisement:
1. Increment the hop count (Metric).
2. If the new Metric is lower than the existing entry for that Region, update the Routing Table.
3. If the Metric is equal, use **Trust Scoring** as a tie-breaker.
4. Apply **Split Horizon**: Do not propagate a route back to the interface/peer from which it was learned.

### 4.3 Spatial Trust Scoring
Trust is a dynamic value $[0.0, 1.0]$ calculated as:
$$Trust = (Base 	imes Decay^{\Delta t}) + ProximityBoost$$
- **Decay:** Trust slowly diminishes if no advertisements are received.
- **Proximity Boost:** Routes from nodes in the same or adjacent Hilbert Regions receive a significantly higher trust weight.

## 5. Hardware Abstraction (GPU Integration)
The Routing Table is mirrored in **Shared RAM** (RAM[2048-2559]) to allow GPU-resident processes to make sub-millisecond routing decisions without CPU intervention.

| RAM Offset | Content |
|------------|---------|
| 2048 | Local Node Address |
| 2049-2304 | Routing Table (Region -> {Dist, Gateway}) |
| 2305-2560 | Peer Table (Active Neighbors) |

## 6. GeoASM Opcodes
| Opcode | G-Channel | B-Channel | Description |
|--------|-----------|-----------|-------------|
| `ROUTE` | 0xB0 | `addr` | Register a route to the target address. |
| `FWD` | 0xB1 | `addr` | Forward a packet to the target address. |
| `BCAST` | 0xB3 | `region` | Broadcast a packet to an entire Hilbert Region. |
