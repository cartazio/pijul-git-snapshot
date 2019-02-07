#!/usr/bin/env bats

load ../test_helper

@test "break_context_twice" {
    make_single_file_repo a toto
    pijul clone a b
    pijul clone a c
    echo -n > a/toto
    echo -n > b/toto
    append_random c/toto

    pijul record --repository a -a -m msg -A me
    pijul record --repository b -a -m msg -A me
    pijul record --repository c -a -m msg -A me

    pijul pull -a --repository a b
    pijul pull -a --repository a c
    pijul pull -a --repository b a
    pijul pull -a --repository c a

    assert_files_equal a/toto b/toto
    assert_files_equal a/toto c/toto

    if [[ $(wc -l a/toto) -ne "0" ]]; then
        return 1
    fi
    if [[ $(grep ">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>" a/toto | wc -l) -ne "1" ]]; then
        return 1
    fi
}
