#!/usr/bin/env bats

load ../test_helper


@test "conflicts down context" {
    DATE="2017-12-14T14:06:12+01:00"
    mkdir a
    pijul init a
    touch a/file
    pijul add --repository a file
    pijul record -a --repository a -m "file" -A Alice --date "$DATE"

    pijul clone a b
    echo a > a/file
    pijul record -a --repository a -m "a" -A Alice --date "$DATE"

    echo b > b/file
    pijul record -a --repository b -m "b" -A Bob --date "$DATE"

    pijul pull -a --repository a b

    echo x > file
    cat a/file >> file
    mv file a/file
    pijul record -a --repository a -m "resolution" -A resolver --date "$DATE"

    pijul clone a c

    diff a/file c/file
}
