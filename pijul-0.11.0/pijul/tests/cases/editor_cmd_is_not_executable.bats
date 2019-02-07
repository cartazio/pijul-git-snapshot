#!/usr/bin/env bats

load ../test_helper

# given editor is not executable,
# then pijul fails with improved error message.

@test "editor command is not executable" {
    touch editor.sh
    make_repo a

    write_meta_file a <<EOF
editor = "../editor.sh"
EOF

    make_random_file "a/file.txt"
    pijul add --repository a file.txt
    run pijul record -a -A me --repository a
    assert_failure "Cannot start editor \"../editor.sh\" (\"Permission denied (os error 13)\")"
}
