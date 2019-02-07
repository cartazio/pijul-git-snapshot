#!/usr/bin/env bats

load ../test_helper

# given editor is whitespace,
# then pijul fails with improved error message.

@test "editor command is whitespace" {
    make_repo a

    write_meta_file a <<EOF
editor = " "
EOF

    make_random_file "a/file.txt"
    pijul add --repository a file.txt
    run pijul record -a -A me --repository a
    assert_failure "Cannot start editor \" \" (\"No such file or directory (os error 2)\")"
}
