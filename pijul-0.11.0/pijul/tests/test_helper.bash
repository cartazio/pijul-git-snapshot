run_only_test() {
  if [ "$BATS_TEST_NUMBER" -ne "$1" ]; then
    skip
  fi
}


setup() {
  # To run only one test, uncomment the next line.
  # run_only_test 20

  export PIJUL_SRC_DIR="$BATS_TEST_DIRNAME/../.."
  # TODO: this is currently only for testing debug builds, but it should be configurable.
  if [ -z $PIJUL_EXE ]; then
    export PIJUL_EXE="$PIJUL_SRC_DIR/../target/debug/pijul"
  fi

  # Make a clean tmpdir for putting repos in (hopefully, it won't be put in an
  # existing pijul repo).
  export PIJUL_REPO_DIR=$(mktemp -d)
  cd $PIJUL_REPO_DIR

  # Also test "debug!" lines (increases coverage for free)
  # export RUST_LOG="pijul=debug,libpijul=debug"

  # Since the home directory might contain a pijul configuration file, make
  # sure we start with a clean home directory.

  # (actually, we might want it sometimes, for instance for SSH).
  export HOME_BACKUP=$HOME
  export PIJUL_HOME=$(mktemp -d)
  export HOME=$PIJUL_HOME
}

cleanup() {
    rm -Rf $PIJUL_REPO_DIR
    rm -Rf $PIJUL_HOME
}

pijul() {
    if [ -z "$PIJUL_USE_KCOV" ]; then
	"$PIJUL_EXE" "$@"
    else
        mkdir -p "$PIJUL_SRC_DIR/kcov"
        DIR=$(mktemp -d -p "$PIJUL_SRC_DIR/kcov" "$BATS_TEST_NAME.XXXXXXXX")
	kcov --include-path="$PIJUL_SRC_DIR/.." --exclude-path=/home/pe/.cargo "$DIR" "$PIJUL_EXE" "$@"
    fi
}

pijul_uncovered() {
    "$PIJUL_EXE" "$@"
}

teardown() {
    rm -rf "$PIJUL_REPO_DIR"
    rm -rf "$PIJUL_HOME"
}

make_two_repos() {
    make_repo "$1"
    make_repo "$2"
}

make_repo() {
    mkdir "$1"
    pijul init "$1"
}

# make_single_file_repo dirname filename
make_single_file_repo() {
    mkdir "$1"
    pijul init "$1"
    make_random_file "$1"/"$2"
    pijul add --repository "$1" "$2"
    pijul record -a --repository "$1" -m msg -A me
}

make_random_file() {
    cat /dev/urandom | tr -dc 'a-zA-Z0-9' | fold -w 80 | head -n 10 > "$1"
}

append_random() {
    cat /dev/urandom | tr -dc 'a-zA-Z0-9' | fold -w 80 | head -n 2 >> "$1"
}

prepend_random() {
    NAME=$(mktemp)
    mv "$1" $NAME
    cat /dev/urandom | tr -dc 'a-zA-Z0-9' | fold -w 80 | head -n 2 >> "$1"
    cat $NAME >> "$1"
}

# use heredocs to specify contents or <<<"" to clear.
write_meta_file() {
    local repo_dir="$1"
    cat - > "$repo_dir"/.pijul/meta.toml
}

assert_success() {
  if [[ "$status" -ne 0 ]]; then
    echo "command failed with exit status $status"
    return 1
  elif [[ "$#" -gt 0 ]]; then
    assert_output "$1"
  fi
}

assert_failure() {
  if [[ "$status" -eq 0 ]]; then
    echo "expected failed exit status"
    return 1
  elif [[ "$#" -gt 0 ]]; then
      assert_output "$1"
  fi
}

assert_output() {
    if grep -q "$1" <(echo "$output"); then
	echo "success"
	return 0
    else
	echo "expected: $1"
	echo "actual: $output"
	return 1
    fi
}

assert_number_lines() {
    nlines=$(echo "$output" | wc -l)
    if [[ $nlines -eq $1 ]]; then
       echo "success"
       return 0
    else
	echo "expected: $1 lines"
	echo "actual $nlines lines:"
	echo "$output"
	return 1
    fi
}

assert_empty() {
    nlines=$(echo -n "$output" | wc -l)
    if [[ $nlines -eq 0 ]]; then
       echo "success"
       return 0
    else
	echo "expected: $1 lines"
	echo "actual $nlines lines:"
	echo "$output"
	return 1
    fi
}

assert_files_equal() {
  cmp "$1" "$2"
  if [[ $? -ne 0 ]]; then
    echo "files should be the same"
    echo "first file:"
    cat "$1"
    echo "second file:"
    cat "$2"
    return 1
  fi
}

assert_file_contains() {
  grep --quiet "$2" "$1"
  if [[ $? -ne 0 ]]; then
    echo "file $1 was supposed to contain $2"
    return 1
  fi
}

assert_dirs_equal() {
  diff --exclude=.pijul -u -r "$1" "$2"
  if [[ $? -ne 0 ]]; then
    echo "error comparing directories"
    return 1
  fi
}
