; XOR Neural Network Weights
; Architecture: 2 -> 4 -> 1
; Fixed-point 16.16 format

; === Layer 1 Weights (2x4) ===
  LDI r1, 305061  ; w1[0][0] = 4.6549
  STORE r1, 4352
  LDI r1, 44506  ; w1[0][1] = 0.6791
  STORE r1, 4353
  LDI r1, 129866  ; w1[0][2] = 1.9816
  STORE r1, 4354
  LDI r1, 430115  ; w1[0][3] = 6.5630
  STORE r1, 4355
  LDI r1, 273993  ; w1[1][0] = 4.1808
  STORE r1, 4356
  LDI r1, 4294815155  ; w1[1][1] = -2.3215
  STORE r1, 4357
  LDI r1, 135738  ; w1[1][2] = 2.0712
  STORE r1, 4358
  LDI r1, 443906  ; w1[1][3] = 6.7735
  STORE r1, 4359

; === Layer 1 Bias (4) ===
  LDI r1, 4294525248  ; b1[0] = -6.7451
  STORE r1, 4448
  LDI r1, 4294931312  ; b1[1] = -0.5491
  STORE r1, 4449
  LDI r1, 30568  ; b1[2] = 0.4664
  STORE r1, 4450
  LDI r1, 4294775550  ; b1[3] = -2.9258
  STORE r1, 4451

; === Layer 2 Weights (4x1) ===
  LDI r1, 4294301149  ; w2[0][0] = -10.1646
  STORE r1, 4864
  LDI r1, 86142  ; w2[1][0] = 1.3144
  STORE r1, 4865
  LDI r1, 4294959517  ; w2[2][0] = -0.1187
  STORE r1, 4866
  LDI r1, 648150  ; w2[3][0] = 9.8900
  STORE r1, 4867

; === Layer 2 Bias (1) ===
  LDI r1, 4294646068  ; b2[0] = -4.9015
  STORE r1, 4928

; === Weight addresses ===
; INPUT_ADDR  = 0x1000
; W1_ADDR     = 0x1100
; B1_ADDR     = 0x1160
; HIDDEN_ADDR = 0x1200
; W2_ADDR     = 0x1300
; B2_ADDR     = 0x1340
; OUTPUT_ADDR = 0x1400