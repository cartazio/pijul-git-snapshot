#!/usr/bin/env bats

load ../test_helper

# For this script to work, `ssh localhost` must connect without asking
# anything.

@test "remote (ssh)" {
    mkdir subdir
    cd subdir
    pijul_uncovered init
    mkdir a b
    echo initial_a >> a/file.txt
    echo initial_b >> b/file.txt
    pijul_uncovered record -a -n -A edith -m 'create two files and dirs'
    echo addition_a >> a/file.txt
    echo addition_b >> b/file.txt
    pijul_uncovered record -a -n -A me -m 'edit two files in dirs'

    # The following definition is intentionally stupid: indeed, why
    # not define this variable at the beginning of this test, instead
    # of relative to the current directory? This actually tests
    # find_repo_root's use of canonical paths.
    REMOTE_DIR=$(pwd)/../remote
    mkdir -p $REMOTE_DIR

    export HOME=$HOME_BACKUP
    RUST_LOG=pijul=debug REMOTE_PIJUL="$PIJUL_EXE" pijul clone . localhost:$REMOTE_DIR &> /tmp/log

    diff $REMOTE_DIR/a/file.txt a/file.txt
    diff $REMOTE_DIR/b/file.txt b/file.txt

    cd $REMOTE_DIR
    echo blabla >> a/file.txt
    pijul_uncovered record -a -n -A me -m "remote"

    cd $PIJUL_REPO_DIR/subdir
    REMOTE_PIJUL="$PIJUL_EXE" pijul pull -p 22 -a localhost:$REMOTE_DIR --set-default
    echo blibli >> a/file.txt
    pijul_uncovered key gen --signing --local
    pijul_uncovered record -a -n -A me -m 'blibli'
    REMOTE_PIJUL="$PIJUL_EXE" pijul push -p 22 -a

    cd $REMOTE_DIR
    echo blibli >> a/file.txt
    pijul_uncovered record -a -n -A me -m "http"
    python -m http.server 8000&
    HTTP=$!
    trap "$(trap -p EXIT); kill -9 $HTTP" EXIT
    sleep 1
    cd $PIJUL_REPO_DIR/subdir
    pijul pull -a http://localhost:8000
    pijul pull -a file://$REMOTE_DIR/repo

    OTHER_CLONE=$(pwd)/../other_clone
    cd $OTHER_CLONE
    pijul clone localhost:$REMOTE_DIR/repo
}
