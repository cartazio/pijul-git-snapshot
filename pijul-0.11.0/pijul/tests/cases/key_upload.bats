#!/usr/bin/env bats

load ../test_helper

@test "Key upload" {
    export HOME=$HOME_BACKUP
    REMOTE_PIJUL="$PIJUL_EXE" pijul key upload $USER@localhost
}
