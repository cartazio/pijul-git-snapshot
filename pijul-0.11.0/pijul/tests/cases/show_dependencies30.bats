#!/usr/bin/env bats

load ../test_helper


@test "show dependencies (30 patches)" {
    mkdir subdir
    touch subdir/file.txt
    pijul init subdir
    pijul add --repository subdir file.txt
    pijul record -a --repository subdir -m "add file.txt" -A you
    echo bla > subdir/file.txt
    pijul record -a --repository subdir -m "modify file.txt" -A me
    touch subdir/file2.txt
    pijul add --repository subdir file2.txt
    pijul record -a --repository subdir -m "add file2.txt" -A them
    pijul log --repository subdir --hash-only > /tmp/blu

    for i in $(seq 1 30); do
	echo "modification number $i" > subdir/file.txt
	pijul record -a --repository subdir -m "modify file.txt, take $i" -A me
    done
    latest_patch=$(pijul log --repository subdir --hash-only | grep ": 0$" | cut -d: -f 1)

    pijul show-dependencies --repository subdir $lastest_patch > out
    num_deps=$(grep -c -e "->" out)
    echo "num_deps: $num_deps"
    [[ $num_deps -eq 31 ]]
}
