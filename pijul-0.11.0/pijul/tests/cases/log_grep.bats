#!/usr/bin/env bats

load ../test_helper

@test "diff" {
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
    pijul log --grep edit
    n_edit_patches=$(pijul log --grep edit --hash-only | tail -n +2 | wc -l)
    echo $n_edit_patches
    if [[ $n_edit_patches -ne "1" ]]; then
	return 1
    fi
    n_all_patches=$(pijul log --hash-only | tail -n +2 | wc -l)
    if [[ $n_all_patches -ne "2" ]]; then
	return 1
    fi
}
