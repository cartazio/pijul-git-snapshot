#!/usr/bin/env bats

load ../test_helper

@test "unrecord conflict resolution" {
    mkdir a

    cd a
    pijul init

    echo "a\nb\nc" > file
    pijul add file
    pijul record -a -m "Add file" -A "Me"

    cd ..
    pijul clone a left
    cd left
    echo "a\nb\nleft\nleft\nc" > file
    pijul record -a -m "Edit file left" -A left

    cd ..
    pijul clone a right
    cd right
    echo "a\nb\nright\nright\nc" > file
    pijul record -a -m "Edit file right	" -A right
    pijul pull ../left -a
    run pijul status -s
    assert_output "C file"
    echo "a\nb\nleft\nright\nright\nleft\nc" > file
    pijul record -a -m "Resolve conflict right" -A right
    run pijul status -s
    assert_output "^$"
    assert_number_lines 1
    echo yd | pijul unrecord
    run pijul status -s
    assert_output "C file"
}
