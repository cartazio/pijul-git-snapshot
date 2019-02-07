#!/usr/bin/env bats

load ../test_helper

@test "diff" {
    mkdir subdir
    cd subdir
    pijul init
    mkdir a b
    echo initial_a >> a/file.txt
    echo initial_b >> b/file.txt
    RUST_LOG="pijul=debug" pijul record -a -n -A me -m 'create two files and dirs' 2> /tmp/log
    echo addition_a >> a/file.txt
    echo addition_b >> b/file.txt
    n_touched_files=$(pijul diff | grep "In file" | wc -l)
    if [[ $n_touched_files -ne "2" ]]; then
	return 1
    fi
    n_touched_files_a=$(pijul diff a | grep "In file" | wc -l)
    if [[ $n_touched_files_a -ne "1" ]]; then
	return 1
    fi
}
