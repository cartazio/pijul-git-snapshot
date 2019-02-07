#!/usr/bin/env bats

load ../test_helper

@test "unrecord file edit" {
    mkdir a

    cd a
    pijul init

    cat > file <<EOF
a
b
c
EOF
    pijul add file
    pijul record -a -m "Add file" -A "Me"
    cat > file <<EOF
a
b
1
c
EOF
    pijul record -a -m "Edit file 1" -A left
    cat > file <<EOF
a
b
2
2
c
EOF
    pijul record -a -m "Edit file 2" -A right
    echo yd | pijul unrecord
    pijul revert -a
    cat file
    grep 1 file
}
