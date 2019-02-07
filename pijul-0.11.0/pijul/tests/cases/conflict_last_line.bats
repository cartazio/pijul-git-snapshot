#!/usr/bin/env bats

load ../test_helper


@test "conflicts last line" {
    DATE="2017-12-14T14:06:12+01:00"
    mkdir a
    pijul init a
    touch a/file
    pijul add --repository a file
    pijul record -a --repository a -m "file" -A Alice --date "$DATE"

    pijul clone a b
    echo -n a > a/file
    pijul record -a --repository a -m "a" -A Alice --date "$DATE"

    echo -n b > b/file
    pijul record -a --repository b -m "b" -A Bob --date "$DATE"

    pijul pull -a --repository a b

    run tail -n 1 a/file
    assert_output "^<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<$"

    run pijul diff --repository a
    assert_empty
}
