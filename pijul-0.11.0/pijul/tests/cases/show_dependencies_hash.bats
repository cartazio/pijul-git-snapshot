#!/usr/bin/env bats

load ../test_helper


@test "show dependencies with hash" {
    mkdir subdir
    touch subdir/file.txt
    pijul_uncovered init subdir
    pijul_uncovered add --repository subdir file.txt
    HASH=$(pijul_uncovered record -a --repository subdir -m "add file.txt" -A you | sed -e 's/Recorded patch //')
    echo bla > subdir/file.txt
    pijul_uncovered record -a --repository subdir -m "modify file.txt" -A me
    touch subdir/file2.txt
    pijul_uncovered add --repository subdir file2.txt
    HASH2=$(pijul_uncovered record -a --repository subdir -m "add file2.txt" -A them | sed -e 's/Recorded patch //')

    pijul_uncovered show-dependencies --repository subdir > out
    num_deps=$(grep -c -e "->" out)
    [[ $num_deps == 1 ]]

    pijul show-dependencies --repository subdir --depth 3 $HASH2 > out
    ! grep -c -e "->" out

}
