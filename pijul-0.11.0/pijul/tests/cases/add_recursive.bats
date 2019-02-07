#!/usr/bin/env bats

load ../test_helper

@test "add a directory recursively" {
    pijul_uncovered init
    mkdir -p a/b/c/d
    touch a/b/c/d/file.txt
    pijul add --recursive a
    pijul ls | grep file.txt
    if [[ "$status" -ne 0 ]]; then
      echo "command failed with exit status $status"
      return 1
    fi
}
