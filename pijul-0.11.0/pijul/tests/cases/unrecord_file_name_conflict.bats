#!/usr/bin/env bats

load ../test_helper

@test "unrecord file name conflict" {
    mkdir a b

    cd a
    pijul_uncovered init

    echo a > file
    pijul_uncovered add file
    pijul_uncovered record -a -m "a" -A "Me"

    cd ../b
    pijul_uncovered init
    echo b > file
    pijul_uncovered add file
    pijul_uncovered record -a -m "b" -A "Me"

    pijul_uncovered pull -a ../a
    INT=$(pijul_uncovered log | grep Internal | sed -e "s/.*Internal id:.* \(.*\)/\1/" | head -n 1)
    ls > /tmp/ls
    pijul_uncovered log > /tmp/log
    echo ">>file.$INT" >> /tmp/ls
    pijul_uncovered remove file.$INT
    echo yd | pijul unrecord
    pijul_uncovered revert -a
    if [[ $(pijul_uncovered ls) != "file" ]]; then
        return 1
    fi
    ls file
}
