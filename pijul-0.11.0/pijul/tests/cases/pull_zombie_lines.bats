#!/usr/bin/env bats

load ../test_helper

@test "pull zombie lines" {
    make_single_file_repo a toto
    pijul_uncovered clone a b

    echo -n > a/toto
    append_random b/toto_append
    cat b/toto_append >> b/toto
    cp b/toto_append b_toto_append
    pijul_uncovered record --repository a -a -m msg -A me
    pijul_uncovered record --repository b -a -m msg -A me
    pijul pull -a --repository b a
    pijul pull -a --repository a b

    cp b/toto_append /tmp/b_toto_append
    assert_files_equal a/toto b/toto

    pijul_uncovered clone a c
    pijul_uncovered clone a d
    rm -Rf /tmp/a /tmp/b
    pijul_uncovered clone a /tmp/a
    pijul_uncovered clone b /tmp/b
    grep ">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>" b/toto

    # fixing the conflict by keeping the lines
    cp b_toto_append b/toto

    pijul record --repository b -a -m "fix: keep" -A me
    pijul pull -a --repository a b
    ! grep ">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>" a/toto
    if [[ $? -ne 0 ]]; then
      echo "Fixing the conflict by keeping the lines failed"
      return 1
    fi

    # fixing the conflict by dropping the lines
    grep ">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>" c/toto
    echo -n > c/toto
    pijul record --repository c -a -m "fix: drop" -A me
    pijul pull -a --repository d c
    ! grep ">>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>" d/toto
    if [[ $? -ne 0 ]]; then
      echo "Fixing the conflict by dropping the lines failed"
      return 1
    fi

    assert_files_equal a/toto b/toto
}
