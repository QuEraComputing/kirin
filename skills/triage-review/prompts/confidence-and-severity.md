# Confidence and Severity Levels

## Confidence

For each finding, classify confidence:
- **confirmed**: Certain this is an issue. Use for P0/P1.
- **likely**: Probable issue but a design reason could exist. Use for P1/P2.
- **uncertain**: Looks unusual but could be intentional. Use for P2/P3 only.

Do NOT assign P0/P1 to "uncertain" findings. When uncertain, phrase as a question ("Is X intentional? If not, consider Y.").

## Severity

- P0: Must fix (bugs, unsoundness, correctness issues)
- P1: Should fix (significant improvements, design issues)
- P2: Nice to have (minor improvements, ergonomic tweaks)
- P3: Informational (observations, notes for future)

## Soundness-Specific Severity (for Soundness Adversary)

- P0: Undefined behavior reachable through safe code
- P1: Silent data corruption reachable through public API
- P2: Panic through public API that should be a Result
- P3: Invariant gap requiring adversarial construction (not normal use)
