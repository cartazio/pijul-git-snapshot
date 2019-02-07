#!/usr/bin/env bats

load ../test_helper

@test "fork from a patch" {
    pijul init

    echo "change 1" > file.txt
    pijul add file.txt
    pijul record -A me -a -m "change 1"

    echo "change 2" > file.txt
    target="$(pijul record -A me -a -m "change 2" | sed -e 's/Recorded patch //')"

    echo "change 3" > file.txt
    pijul record -A me -a -m "change 3"

    pijul fork --patch "${target}" test

    patch_number=$(("$(pijul log --hash-only | wc -l)" - 1))

    [ ${patch_number} -eq 2 ]
}
