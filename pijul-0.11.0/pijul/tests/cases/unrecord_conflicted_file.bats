#!/usr/bin/env bats

load ../test_helper

@test "unrecord in conflicted file" {
    mkdir a

    cd a
    pijul_uncovered init

    echo "a\nb\nc" > file
    pijul_uncovered add file
    pijul_uncovered record -a -m "Add file" -A "Me"

    cd ..
    pijul clone a left
    cd left
    echo "a\nb\nleft\nleft\nc" > file
    pijul_uncovered record -a -m "Edit file left" -A left

    cd ..
    pijul clone a right
    cd right
    echo "a\nb\nright\nright\nc" > file
    pijul_uncovered record -a -m "Edit file right	" -A right
    pijul pull ../left -a
    run pijul status -s
    assert_output "C file"
    echo ny | pijul unrecord
    cat file
    pijul revert -a
    assert_files_equal file ../left/file
}
