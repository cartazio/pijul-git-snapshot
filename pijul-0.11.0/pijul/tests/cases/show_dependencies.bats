#!/usr/bin/env bats

load ../test_helper


@test "show dependencies" {
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

    pijul show-dependencies --repository subdir --depth 10000 > log
    num_deps=$(grep -c -e "->" log)
    [[ $num_deps == 1 ]]

    pijul_uncovered show-dependencies --repository subdir $HASH2 > log
    num_deps=$(grep -c -e "->" log)
    [[ $num_deps == 1 ]]

    pijul_uncovered show-dependencies --repository subdir $HASH > tmp_file
    if [[ $(grep -c -e "->" tmp_file) -ne 1 ]]; then
        return 1
    fi
}
