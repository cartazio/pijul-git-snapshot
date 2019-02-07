#!/usr/bin/env bats

load ../test_helper

@test "conflict without epsilon" {
    mkdir a

    cd a
    pijul init
    echo -e "a\nb" > file
    pijul add file
    pijul record -a -m "+file" -A "me"
    cd ..

    pijul clone a b
    cd b
    echo -e "a\nx\nb" > file
    pijul record -a -m "+x" -A "me"

    cd ../a
    echo -n > file
    pijul record -a -m " -ab" -A "me"
    pijul pull ../b -a

    cp file /tmp
    echo b > file
    pijul record -a -m "conflict resolution" -A "me"
    pijul revert -a
    echo b > file2
    cp file file2 /tmp
    diff file file2
    if [[ $? -ne 0 ]]; then
        echo "revert did something after record"
        return 1
    fi
}
