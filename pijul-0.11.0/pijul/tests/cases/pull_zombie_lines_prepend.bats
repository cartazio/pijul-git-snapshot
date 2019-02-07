#!/usr/bin/env bats

load ../test_helper

@test "pull zombie lines with prepended lines" {
    mkdir a
    cd a
    pijul_uncovered init
    echo -e "b\nc\nd" > toto
    pijul_uncovered add toto
    pijul_uncovered record -a -m "+toto" -A "Me"
    cd ..

    pijul clone a b

    echo -e "d" > a/toto
    echo -e "a\nb\nc\nd" > b/toto
    cp b/toto b_toto
    pijul_uncovered record --repository a -a -m msg -A me
    pijul_uncovered record --repository b -a -m msg -A me
    RUST_LOG="libpijul::apply=debug" pijul pull -a --repository b a 2> /tmp/logb
    RUST_LOG="libpijul::apply=debug" pijul pull -a --repository a b 2> /tmp/loga
    cp a/toto /tmp/a_toto
    cp b/toto /tmp/b_toto

    cd a
    pijul_uncovered info --debug
    mv debug_master /tmp/debug_a
    cd ..
    cd b
    pijul_uncovered info --debug
    mv debug_master /tmp/debug_b
    cd ..

    assert_files_equal a/toto b/toto

    pijul_uncovered clone a c
    pijul_uncovered clone a d
    # fixing the conflict by keeping the lines
    cp b_toto b/toto
    pijul record --repository b -a -m "fix: keep" -A me
    pijul pull -a --repository a b 2> /tmp/log0
    ! grep ">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>" a/toto
    if [[ $? -ne 0 ]]; then
      echo "Fixing the conflict by keeping the lines failed"
      return 1
    fi

    # fixing the conflict by dropping the lines
    echo -n > c/toto
    RUST_LOG="libpijul::optimal_diff=debug,libpijul::apply=debug" pijul record --repository c -a -m "fix: drop" -A me 2> /tmp/logc
    cd c
    pijul_uncovered info --debug
    mv debug_master /tmp/debug_c
    cd ..
    RUST_LOG="libpijul::apply=debug" pijul pull -a --repository d c 2> /tmp/logd
    cp d/toto /tmp/d_toto
    ! grep ">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>" d/toto
    if [[ $? -ne 0 ]]; then
      echo "Fixing the conflict by dropping the lines failed"
      return 1
    fi

    assert_files_equal a/toto b/toto
}
