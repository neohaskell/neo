# Next Step

## Goal
Final Polish and Manual Verification.

## Action Items
1.  **Terminal Size Check**:
    - Implement a check in `main.rs` or `output.rs` to ensure the terminal is at least 60 columns wide.
    - Display a branded warning message if the terminal is too narrow.
2.  **Refine TUI Animations**:
    - Ensure all spinners use a consistent frame rate (80ms).
    - Add "success" animations or transitions when completing major tasks.
3.  **Documentation & Help**:
    - Add detailed `long_about` and `help` strings to all subcommands in `cli.rs`.
    - Ensure all `NeoError` variants have helpful `diagnostic` hints. (Already improved SubprocessError)
4.  **Manual Verification**:
    - Run the full suite in a narrow terminal.
    - Verify `--ci` output matches the expected plain-text format for all commands.
