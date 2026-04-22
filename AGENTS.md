# Agent Instructions for NeoCLI

You are an autonomous AI coding agent assigned to implement features for the NeoCLI project. 

Follow this strict workflow for every session:

1. **Read Current State**: Begin by reading `STATE.md` to understand the current progress, what has already been scaffolded, and the overall context of the project.
2. **Execute Next Step**: Read `NEXT_STEP.md` to find your current objective and action items. This is your primary goal.
3. **Consult Implementation Plan**: As you implement the next step, continuously reference `IMPLEMENTATION_PLAN.md` to understand the architectural rules, expected file structures, TUI guidelines, and error handling strategies. This document is your technical source of truth for *how* the feature should be implemented.
4. **Verify Your Work**: Run `cargo check` (and `cargo test` if applicable) to ensure your implementation compiles and is structurally sound.
5. **Update State Log**: Once you have successfully completed the tasks in `NEXT_STEP.md`, append a brief log entry to the bottom of the `STATE.md` file summarizing what you implemented, fixed, and verified. 
6. **Determine Next Step**: Update `NEXT_STEP.md` with the subsequent logical task from `IMPLEMENTATION_PLAN.md` so the next agent session knows exactly what to pick up.

**Key Reminders**:
- Adhere to the `miette` error handling conventions defined in `src/errors.rs`.
- Respect the `OutputMode` abstraction (Interactive vs. CI) detailed in the plan.
- The project strictly uses `tokio` for async and `ratatui` for interactive terminal UI.
