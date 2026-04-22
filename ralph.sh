#!/usr/bin/env bash

MAX_ITER="${1:-10}"
ITER=1

echo "🚀 Starting RALPH Loop (Max iterations: $MAX_ITER)"

while [ $ITER -le $MAX_ITER ]; do
    echo "========================================"
    echo "🔄 Iteration $ITER / $MAX_ITER"
    echo "========================================"
    
    # 1. Verify Test Coverage
    echo "🕵️  [Agent] Verifying test coverage for completed tasks..."
    gemini -p "Please review STATE.md and verify that all completed features have comprehensive unit tests and integration tests. If any tests are missing for what has been implemented, write them now and verify they pass with 'cargo test'." --approval-mode yolo

    # 2. Run the agent
    echo "🤖 [Agent] Executing workflow..."
    
    if [ -f .test_failures.log ]; then
        echo "⚠️ Found test failures from previous iteration. Asking agent to fix them."
        gemini -p "The previous build/tests failed. Please read .test_failures.log, fix the issues in the code, and then verify with 'cargo check' and 'cargo test'. Continue following the standard workflow in AGENTS.md." --approval-mode yolo
        rm .test_failures.log
    else
        gemini -p "Please execute the workflow defined in AGENTS.md. Read the current STATE.md and NEXT_STEP.md, implement the changes according to IMPLEMENTATION_PLAN.md, and then update STATE.md and NEXT_STEP.md." --approval-mode yolo
    fi
    
    # 3. Implement Tests
    echo "🤖 [Agent] Implementing tests..."
    gemini -p "Please implement comprehensive unit tests and integration tests for the features you just built. Verify they run locally using 'cargo test'." --approval-mode yolo
    
    # 4. Verify / Test
    echo "🧪 [Test] Verifying build and tests..."
    
    # We run the tests and capture output
    if cargo check > .cargo_check.log 2>&1 && cargo test > .cargo_test.log 2>&1; then
        echo "✅ Tests passed!"
        rm -f .cargo_check.log .cargo_test.log
        
        # Check if the agent marked the task as done
        # Heuristic: if NEXT_STEP.md contains "done" or "complete"
        # We can just let it loop, or break if NEXT_STEP.md is empty or explicitly says "DONE"
        if grep -q -i "^done" NEXT_STEP.md; then
            echo "🎉 Agent indicated completion in NEXT_STEP.md. Exiting RALPH loop."
            break
        fi
    else
        echo "❌ Tests failed! Saving logs for the next iteration to fix."
        cat .cargo_check.log .cargo_test.log > .test_failures.log
        rm -f .cargo_check.log .cargo_test.log
        
        echo "--- Test Failure Output ---"
        cat .test_failures.log
        echo "---------------------------"
    fi
    
    ITER=$((ITER + 1))
    echo "⏳ Waiting a moment before the next iteration..."
    sleep 2
done

echo "🏁 RALPH Loop finished."
