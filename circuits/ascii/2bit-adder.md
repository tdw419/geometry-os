# 2-Bit Adder (Simplified)

Uses two half-adders with carry propagation.

## ASCII Layout

```
        A1    B1            A0    B0
        │     │             │     │
        │     │             │     │
        └──┬──┘             └──┬──┘
           │                   │
           X                   X
          ┌┴┐                 ┌┴┐
          │ &│                 │ &│
          └┬┘                 └┬┘
           │                    │
     C1 ───┼────────────────────┼─── C0
           │                    │
          S1                   S0
```

## Simplified 2-bit Adder ASCII

```
   A1 B1       A0 B0
    │  │        │  │
    └──┘        └──┘
     X           X
    ┌┴┐         ┌┴┐
    │&│         │&│
    └┬┘         └┬┘
     │           │
─────┼───────────┼─────
     │           │
    S1          S0
```
