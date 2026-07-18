// src/lib/pin-constants.ts
//
// Shared PIN length bounds. Kept in one place so PinSetup (creation) and
// PinVerify (entry) enforce the same limits the backend validates.

export const MIN_PIN_LEN = 6;
export const MAX_PIN_LEN = 32;
