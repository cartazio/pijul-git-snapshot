#!/usr/bin/env bats

load ../test_helper

@test "interactive pull" {
    make_two_repos titi toto
    cd titi
    make_random_file a
    pijul add a
    pijul record -a -m "a_a_aa_aaa_aaaaa" -A a
    make_random_file b
    pijul add b
    pijul record -a -m "b_bb_bbbb_bbbbbbbb" -A b
    cd ../toto
    echo yn | pijul pull ../titi
    run pijul log
    assert_output "a_a_aa_aaa_aaaaa"
}
