#!/usr/bin/env bats

load ../test_helper


check_blame() {
    authors=$(tail -n +2 $1 | cut -d' ' -f 1)
    line_ends=$(tail -n +2 $1 | cut -d' ' -f 1)
    [ "$authors" = "$line_ends" ]
}

@test "blame" {
    mkdir subdir
    touch subdir/file.txt
    pijul init subdir
    pijul add --repository subdir file.txt
    pijul record -a --repository subdir -m "add file.txt" -A creator
    cat > subdir/file.txt << EOF
me
me
me
me
me
EOF
    pijul record -a --repository subdir -m "modify file.txt" -A me
    cat > subdir/file.txt << EOF
me
you
you
you
me
EOF
    pijul record -a --repository subdir -m "modify file.txt again" -A you
    cat > subdir/file.txt << EOF
me
you
them
you
me
EOF
    pijul record -a --repository subdir -m "modify file.txt thrice" -A them
    cat > subdir/file.txt << EOF
me
you
them
really me
me
EOF
    pijul record -a --repository subdir -m "modify file.txt for good" -A me

    pijul credit --repository subdir file.txt > log
    head -n 1 log | grep creator
    check_blame $out
}
