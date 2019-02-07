#!/usr/bin/env bats

load ../test_helper

@test "break_context_add_twice" {
    make_single_file_repo a toto
    pijul clone a b
    pijul clone a c
    echo -n > a/toto
    append_random b/toto
    append_random c/toto

    pijul record --repository a -a -m "break a" -A me
    pijul record --repository b -a -m "newnodes b" -A me
    pijul record --repository c -a -m "newnodes c" -A me

    pijul pull -a --repository a b
    RUST_LOG="libpijul::optimal_diff=debug,libpijul::graph=debug" pijul pull -a --repository a c 2> /tmp/log
    pijul pull -a --repository b a
    pijul pull -a --repository c a

    assert_files_equal a/toto b/toto
    assert_files_equal a/toto c/toto
}
