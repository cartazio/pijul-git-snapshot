#!/usr/bin/env bats

load ../test_helper

@test "unrecord deep file name conflict" {
    mkdir -p a/x/y/z b/x/y/z

    cd a
    pijul_uncovered init
    echo a > file
    pijul_uncovered add file x/y/z
    pijul_uncovered record -a -m "+file" -A "Me"

    cd ..
    pijul_uncovered clone a b

    cd b
    pijul_uncovered mv file x/y/z
    pijul_uncovered record -a -m "mv file x/y/z" -A "Me"

    cd ../a
    pijul_uncovered remove x
    pijul_uncovered record -a -m "rm x/y/z" -A "Me"

    pijul_uncovered pull -a ../b

    rm -R *
    pijul revert -a
    ls x/y/z/file || ls file
    pijul_uncovered ls | grep file

    echo yd | pijul unrecord

    pijul_uncovered revert -a
    pijul_uncovered ls | grep "file"
    ls file
}
