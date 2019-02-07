#!/bin/sh
if [[ -z "$PIJUL_USE_KCOV" ]]; then
    echo "If you want to run these tests and gather coverage info, set"
    echo "the environment variable PIJUL_USE_KCOV to something."
else
    echo "Coverage data will be placed in the 'pijul/kcov' directory"
    echo "(relative to the repository root)."
fi
if [[ -z "$@" ]]; then
    echo "Running all tests. If you want to run only some tests, use"
    echo "run_tests.sh cases/XXX.bats"
    if [[ "$BATS_JOBS" != "1" ]]; then
        if [[ -z "$BATS_JOBS" ]]; then
            ls cases/*.bats | parallel ./bats/bats -p
        else
            ls cases/*.bats | parallel -j $BATS_JOBS ./bats/bats -p
        fi
    else
        ./bats/bats cases
    fi
else
    if [[ "$BATS" != "1" ]] && [[ $(echo "$@" | wc -w) -ge 2 ]]; then
        if [[ -z "$BATS_JOBS" ]]; then
            echo $@ | tr ' ' '\n' | parallel ./bats/bats -p
        else
            echo $@ | tr ' ' '\n' | parallel -j $BATS_JOBS ./bats/bats -p
        fi
    else
        ./bats/bats $@
    fi
fi
