#!/usr/bin/env bats

load ../test_helper

@test "revert add" {
    mkdir subdir
    cd subdir
    touch file.txt
    pijul init
    pijul add file.txt
    yes | pijul revert
    if [[ "$(pijul ls | wc -l)" -ne 0 ]]; then
        echo "file.txt wasn't removed:"
        pijul ls
        return 1
    fi
}
