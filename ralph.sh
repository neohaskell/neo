#!/usr/bin/env bash

# ralph.sh - The Manual Testing Focused Development Loop
# Usage: ./ralph.sh [max_iterations]

MAX_ITER="${1:-10}"
ITER=1

echo "🚀 Starting RALPH Loop (Manual Testing Focus)"
echo "📍 Max iterations: $MAX_ITER"

while [ $ITER -le $MAX_ITER ]; do
    echo ""
    echo "================================================================================"
    echo "🔄 Iteration $ITER / $MAX_ITER"
    echo "================================================================================"

    # 1. Prepare Prompt
    # We check if there were previous failures to prioritize fixing them
    if [ -f .test_failures.log ]; then
        EXTRA_INSTRUCTION="The previous iteration had automated test failures. Please read .test_failures.log and FIX them first. Then proceed with the manual testing workflow."
    else
        EXTRA_INSTRUCTION="Focus on the current goal in NEXT_STEP.md."
    fi

    echo "🤖 [Agent] Invoking Ralph..."

    # 2. Run the agent
    # We use tee to capture output for the "HUMAN_ASSISTANCE_REQUIRED" check
    gemini -m gemini-3-flash-preview -p "
    You are Ralph, an autonomous coding agent. 
    Follow the workflow in AGENTS.md strictly.
    
    PRIMARY FOCUS: 
    Test the CLI manually by hand in a shell. Run 'cargo run -- <command>' and verify the behavior, TUI, and side effects. 
    Fix any bugs you find during this process immediately. 
    Do not stop until the CLI works as expected for the current task.
    
    $EXTRA_INSTRUCTION
    
    Current Task (from NEXT_STEP.md):
    $(cat NEXT_STEP.md)
    
    IMPORTANT: 
    - If automated tests (cargo check/test or the smoke test) failed, you MUST read .test_failures.log, document the specific errors in NEXT_STEP.md, and then FIX them.
    - If you find a bug that requires human design decisions, or if you are stuck, 
    you MUST include the string 'HUMAN_ASSISTANCE_REQUIRED' in your response and explain why.
    " --approval-mode yolo 2>&1 | tee .agent_output.log

    # 3. Check for Human Assistance Flag
    if grep -q "HUMAN_ASSISTANCE_REQUIRED" .agent_output.log; then
        echo ""
        echo "⚠️  [STOP] Ralph has requested human assistance."
        echo "Please review the output above and provide guidance."
        rm .agent_output.log
        exit 0
    fi
    # We keep .agent_output.log for a bit longer to check for completion

    # 4. Automated Safety Net
    echo "🧪 [Verify] Running automated checks..."
    
    cargo check > .cargo_check.log 2>&1
    CHECK_STATUS=$?
    
    cargo test > .cargo_test.log 2>&1
    TEST_STATUS=$?
    
    # 5. Neo-on-Neo Smoke Test
    echo "🏗️  [Smoke Test] Verifying neo-on-neo workflow..."
    SMOKE_DIR="test-project-smoke"
    rm -rf "$SMOKE_DIR"
    
    # Step 1: New
    cargo run -- new "$SMOKE_DIR" --ci > .smoke_test.log 2>&1
    SMOKE_STATUS=$?
    
    if [ $SMOKE_STATUS -eq 0 ]; then
        # Step 2: Build
        NEO_BIN="$(pwd)/target/debug/neo"
        (cd "$SMOKE_DIR" && "$NEO_BIN" build --ci >> ../.smoke_test.log 2>&1)
        SMOKE_STATUS=$?
    fi
    
    if [ $SMOKE_STATUS -eq 0 ]; then
        # Step 3: Test
        NEO_BIN="$(pwd)/target/debug/neo"
        (cd "$SMOKE_DIR" && "$NEO_BIN" test --ci >> ../.smoke_test.log 2>&1)
        SMOKE_STATUS=$?
    fi

    if [ $SMOKE_STATUS -ne 0 ]; then
        echo "❌ Smoke Test Failed!"
        {
            echo ""
            echo "--- SMOKE TEST FAILURE ---"
            cat .smoke_test.log
        } > .smoke_test_fail.log
    fi
    rm -rf "$SMOKE_DIR" .smoke_test.log

    if [ $CHECK_STATUS -eq 0 ] && [ $TEST_STATUS -eq 0 ] && [ $SMOKE_STATUS -eq 0 ]; then
        echo "✅ Build and automated tests passed!"
        rm -f .cargo_check.log .cargo_test.log .test_failures.log .smoke_test.log
        
        # Check if the agent marked the entire project as done
        if grep -q "ALL_TASKS_COMPLETED" NEXT_STEP.md || grep -q "ALL_TASKS_COMPLETED" .agent_output.log; then
            echo "🎉 ALL TASKS COMPLETED! Exiting RALPH loop."
            break
        fi

        # Heuristic: If NEXT_STEP.md no longer has any unchecked tasks [ ]
        # (Assuming the agent follows the checkbox convention)
        if grep -q "\[ \]" NEXT_STEP.md; then
            echo "⏭️  Moving to next task in NEXT_STEP.md..."
        else
            # If there are no checkboxes at all, but it says 'done'
            if grep -q -i "done" NEXT_STEP.md; then
                 echo "🎉 Task marked as done. Exiting RALPH loop."
                 break
            fi
        fi
    else
        echo "❌ Automated checks failed. Logs saved for next iteration."
        {
            echo "--- CARGO CHECK ---"
            cat .cargo_check.log
            echo ""
            echo "--- CARGO TEST ---"
            cat .cargo_test.log
        } > .test_failures.log
        
        if [ -f .smoke_test_fail.log ]; then
            cat .smoke_test_fail.log >> .test_failures.log
            rm .smoke_test_fail.log
        fi
        
        rm -f .cargo_check.log .cargo_test.log
        
        echo "--- Error Snippet ---"
        tail -n 20 .test_failures.log
        echo "--------------------"
    fi

    ITER=$((ITER + 1))
    rm -f .agent_output.log
    echo "⏳ Cooling down (2s)..."
    sleep 2
done

echo ""
echo "🏁 RALPH Loop finished."
