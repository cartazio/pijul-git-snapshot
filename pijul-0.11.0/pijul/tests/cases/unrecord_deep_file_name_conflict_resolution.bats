#!/usr/bin/env bats

load ../test_helper

@test "unrecord deep file name conflict resolution" {
    mkdir -p a/x/y/z a/u/v/w b/x/y/z  b/u/v/w

    cd a
    pijul_uncovered init
    echo a > u/v/w/file
    pijul_uncovered add x/y/z u/v/w/file
    pijul_uncovered record -a -m "+file" -A "Me"

    cd ..
    pijul_uncovered clone a b

    cd b
    pijul_uncovered mv u/v/w/file x/y/z
    pijul_uncovered record -a -m "mv file x/y/z" -A "Me"

    cd ../a
    pijul_uncovered remove x
    pijul_uncovered record -a -m "rm x/y/z" -A "Me"

    pijul_uncovered pull -a ../b

    pijul_uncovered info --debug --exclude-parents
    cp debug_master /tmp/debug_before

    pijul_uncovered record -a -m "resolution" -A "Me"

    pijul_uncovered info --debug --exclude-parents
    cp debug_master /tmp/debug_a

    cd ../b

    pijul_uncovered pull -a ../a

    pijul_uncovered info --debug
    cp debug_master /tmp/debug_b

    rm -R *
    pijul revert -a
    ls x/y/z/file || ls u/v/w/file
    pijul_uncovered ls | grep file


    # Unrecord just the resolution
    echo yd | pijul unrecord

    pijul_uncovered info --debug
    cp debug_master /tmp/debug_after

    pijul_uncovered revert -a
    pijul_uncovered ls | grep "file"
    ls x/y/z/file
}
