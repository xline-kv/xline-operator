#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common/common.sh"
source "$(dirname "${BASH_SOURCE[0]}")/testenv/testenv.sh"
source "$(dirname "${BASH_SOURCE[0]}")/cases/cases.sh"

function setup() {
  testenv::k8s::kind::create
  testenv::util::install
}

function teardown() {
  testenv::k8s::kind::delete
}

function list_test_cases() {
  local -a functions
  IFS=$'\n' read -d '' -ra functions <<<"$(compgen -A function | sort)" && unset IFS
  local -a testcases=()
  for func in "${functions[@]}"; do
    if [[ "$func" =~ ^test::run:: ]]; then
      testcase=${func#test::run::}
      if [[ -n "${E2E_TEST_CASE_PREFIX:=}" && ${testcase} != "${E2E_TEST_CASE_PREFIX}"* ]]; then
        continue
      fi
      testcases+=("$testcase")
    fi
  done
  echo -n "${testcases[*]}"
}

function run() {
  local -a testcases=()
  IFS=$'\n' read -d '' -ra testcases <<<"$(list_test_cases)" && unset IFS
  local failed=0
  local passed=0
  for testcase in "${testcases[@]}"; do
    log::info "=== Running test case: $testcase ==="
    if ! test::run::"$testcase"; then
      log::error "Test case failed: $testcase"
      ((failed++))
    else
      log::info "Test case passed: $testcase"
      ((passed++))
    fi
  done
  if ((failed > 0)); then
    log::error "Failed test cases: $failed/${#testcases[@]}"
    return "${failed}"
  else
    log::info "All test cases passed"
  fi
}

function help() {
  echo "Xline Operator E2E Test Script"
  echo ""
  echo "Parameters:"
  echo "  -p <prefix>  Run selected test cases with prefix"
  echo "  -h           Print this help"
  echo "  -l           List all test cases"
}

function main() {
  while getopts "p:lh" opt; do
    case "$opt" in
    p)
      export E2E_TEST_CASE_PREFIX="$OPTARG"
      log::info "Run selected test cases with prefix: $E2E_TEST_CASE_PREFIX"
      ;;
    l)
      for testcase in $(list_test_cases); do
        echo "$testcase"
      done
      ;;
    h)
      help
      exit 0
      ;;
    ?) ;;
    esac
  done

  setup || return $?
  run || return $?
  teardown
}

main "$@"
