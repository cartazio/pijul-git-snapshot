#!/usr/bin/env bats

load ../test_helper

@test "unrecord the rollback of a file addition" {
    mkdir a

    cd a
    pijul_uncovered init
    echo -e "a\nb\nc" > file
    pijul_uncovered add file
    pijul_uncovered record -a -m "+file" -A "Me"

    pijul_uncovered remove file
    pijul_uncovered record -a -m " -file" -A "Me"

    echo yd | pijul rollback  -A "me" -m "rollback"
    echo yd | pijul unrecord

    pijul_uncovered ls | grep file
    # a must still be here, ready to be recorded again
    if [[ $? -ne 0 ]]; then
        echo "a is not here"
        return 1
    fi

    pijul_uncovered revert -a
    # a must not be here anymore after revert
    if [[ $(pijul_uncovered ls | wc -l) -ne 0 ]]; then
        echo "a is still here"
        return 1
    fi
}
