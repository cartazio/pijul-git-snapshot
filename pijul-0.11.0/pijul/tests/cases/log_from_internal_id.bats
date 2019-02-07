#!/usr/bin/env bats

load ../test_helper

@test "log from internal id" {
    mkdir subdir
    cd subdir
    pijul_uncovered init
    mkdir a b
    echo initial_a >> a/file.txt
    echo initial_b >> b/file.txt
    pijul_uncovered record -a -n -A edith -m 'create two files and dirs'
    echo addition_a >> a/file.txt
    echo addition_b >> b/file.txt
    pijul_uncovered record -a -n -A me -m 'edit two files in dirs'
    INT=$(pijul_uncovered log | grep "Internal id" | sed -e "s/Internal id: //")
    pijul log --internal-id $INT
}
